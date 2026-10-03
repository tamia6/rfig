//! One editing session. Accepted text is returned to the host shell, never executed here.
use crate::engine::{self, Item, Memory};
use crossterm::{
    cursor::{self, MoveTo, Show},
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyModifiers},
    queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};
use unicode_width::UnicodeWidthStr;

struct CompletionWorker(Option<thread::JoinHandle<()>>);
struct TerminationSignals(Vec<signal_hook::SigId>);
impl Drop for TerminationSignals {
    fn drop(&mut self) {
        for id in self.0.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}
impl Drop for CompletionWorker {
    fn drop(&mut self) {
        crate::cancel_completion();
        if let Some(worker) = self.0.take() {
            let _ = worker.join();
        }
    }
}

struct Screen {
    tty: fs::File,
    x: u16,
    y: u16,
    cols: u16,
    rows: u16,
    drawn: u16,
}
impl Screen {
    fn clear(&mut self) -> io::Result<()> {
        for row in 0..self.drawn {
            queue!(
                self.tty,
                MoveTo(if row == 0 { self.x } else { 0 }, self.y + row),
                ResetColor,
                SetAttribute(Attribute::Reset),
                Clear(ClearType::UntilNewLine)
            )?;
        }
        queue!(self.tty, MoveTo(self.x, self.y), Show)?;
        self.tty.flush()
    }
    fn draw(
        &mut self,
        line: &str,
        point: usize,
        ghost: &str,
        items: &[Item],
        selected: usize,
        pulse: bool,
    ) -> io::Result<()> {
        self.clear()?;
        let (cols, rows) = terminal::size()?;
        self.cols = cols.max(1);
        self.rows = rows.max(1);
        self.x = self.x.min(self.cols - 1);
        self.y = self.y.min(self.rows - 1);
        let line_rows =
            (self.x as usize + UnicodeWidthStr::width(line) + UnicodeWidthStr::width(ghost))
                / self.cols as usize
                + 1;
        let menu_rows = if items.is_empty() {
            0
        } else {
            items.len().min(5) + 1
        };
        let height = (line_rows + menu_rows).min(self.rows as usize) as u16;
        let scroll = (self.y + height).saturating_sub(self.rows);
        if scroll > 0 {
            queue!(
                self.tty,
                MoveTo(0, self.rows - 1),
                Print("\r\n".repeat(scroll as usize))
            )?;
            self.y = self.y.saturating_sub(scroll);
        }
        queue!(
            self.tty,
            MoveTo(self.x, self.y),
            Print(line),
            SetForegroundColor(Color::DarkGrey),
            Print(ghost),
            ResetColor
        )?;
        let first = selected.saturating_sub(4);
        let menu_y = self.y.saturating_add(line_rows as u16);
        for (offset, item) in items.iter().enumerate().skip(first).take(5) {
            let y = menu_y + (offset - first) as u16;
            if y >= self.rows {
                break;
            }
            let active = offset == selected;
            let (icon, color) = match item.kind.as_str() {
                "command" => ("⌘", Color::Cyan),
                "subcommand" => ("↳", Color::Magenta),
                "option" => ("◇", Color::Blue),
                "flag" => ("⚑", Color::Yellow),
                _ => ("●", Color::Green),
            };
            let icon = match item.popular {
                1 => "①",
                2 => "②",
                3 => "③",
                _ => icon,
            };
            let pointer = if active {
                if pulse {
                    " ➜ "
                } else {
                    "→ "
                }
            } else {
                "  "
            };
            let gap = if pulse && active {
                "   "
            } else if pulse && offset.abs_diff(selected) == 1 {
                "  "
            } else {
                " "
            };
            queue!(
                self.tty,
                MoveTo(0, y),
                SetForegroundColor(Color::Cyan),
                Print(pointer),
                SetForegroundColor(color),
                Print(icon),
                ResetColor,
                Print(gap)
            )?;
            if active {
                queue!(
                    self.tty,
                    SetForegroundColor(Color::Cyan),
                    SetAttribute(Attribute::Bold)
                )?;
            } else if pulse && offset.abs_diff(selected) == 1 {
                queue!(self.tty, SetAttribute(Attribute::Bold))?;
            }
            let width = (self.cols as usize)
                .saturating_sub(
                    UnicodeWidthStr::width(pointer) + UnicodeWidthStr::width(icon) + gap.len() + 1,
                )
                .min(41);
            queue!(
                self.tty,
                Print(crate::padded(&item.label, width)),
                ResetColor,
                SetAttribute(Attribute::Reset)
            )?;
        }
        if let Some(item) = items.get(selected) {
            let y = menu_y + items.len().min(5) as u16;
            if y < self.rows {
                queue!(
                    self.tty,
                    MoveTo(0, y),
                    SetForegroundColor(Color::DarkGrey),
                    Print(crate::padded(
                        &item.description,
                        (self.cols as usize).saturating_sub(1)
                    )),
                    ResetColor
                )?;
            }
        }
        self.drawn = height;
        let pos = self.x as usize + UnicodeWidthStr::width(&line[..point]);
        queue!(
            self.tty,
            MoveTo(
                (pos % self.cols as usize) as u16,
                (self.y + (pos / self.cols as usize) as u16).min(self.rows - 1)
            ),
            Show
        )?;
        self.tty.flush()
    }
}
impl Drop for Screen {
    fn drop(&mut self) {
        let _ = self.clear();
        let _ = queue!(
            self.tty,
            DisableBracketedPaste,
            ResetColor,
            SetAttribute(Attribute::Reset),
            Show
        );
        let _ = self.tty.flush();
        let _ = terminal::disable_raw_mode();
    }
}

fn previous(line: &str, point: usize) -> usize {
    line[..point]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0)
}
fn next(line: &str, point: usize) -> usize {
    line[point..]
        .chars()
        .next()
        .map_or(point, |c| point + c.len_utf8())
}

pub fn run(
    shell: String,
    snapshot: PathBuf,
    mut line: String,
    mut point: usize,
    displayed: usize,
) -> io::Result<()> {
    if !matches!(shell.as_str(), "bash" | "fish") || !line.is_char_boundary(point) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid editor arguments",
        ));
    }
    if line.chars().any(char::is_control) {
        fs::write(
            snapshot.join("result"),
            format!(
                "return\n{}\n{line}",
                if shell == "fish" {
                    line[..point].chars().count()
                } else {
                    point
                }
            ),
        )?;
        return Ok(());
    }
    // Exit through the normal cleanup path so completion descendants and raw mode
    // cannot outlive an interrupted editor.
    let terminated = Arc::new(AtomicBool::new(false));
    let mut signals = TerminationSignals(Vec::new());
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGHUP] {
        signals
            .0
            .push(signal_hook::flag::register(signal, terminated.clone())?);
    }
    terminal::enable_raw_mode()?;
    let _raw = crate::RawMode;
    let mut tty = fs::OpenOptions::new().write(true).open("/dev/tty")?;
    queue!(tty, EnableBracketedPaste)?;
    tty.flush()?;
    let (x, y) = match cursor::position() {
        Ok(p) => p,
        Err(e) => {
            let _ = terminal::disable_raw_mode();
            return Err(e);
        }
    };
    let (cols, rows) = terminal::size()?;
    let shown = &line[..displayed.min(line.len())];
    let width = UnicodeWidthStr::width(shown) as u16;
    let offset = (x as usize + cols as usize - (width as usize % cols as usize)) % cols as usize;
    let prior_rows = (offset + width as usize) / cols as usize;
    let mut screen = Screen {
        tty,
        x: offset as u16,
        y: y.saturating_sub(prior_rows as u16),
        cols,
        rows,
        drawn: 1,
    };
    let mut memory = Memory::load(
        engine::config(),
        std::env::var("PWD").unwrap_or_else(|_| {
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        }),
    );
    let names = engine::names(&snapshot);
    let (send, receive) = mpsc::channel::<String>();
    let (results, updates) = mpsc::channel();
    let worker_snapshot = snapshot.clone();
    let shell_is_fish = shell == "fish";
    let worker = CompletionWorker(Some(thread::spawn(move || loop {
        if crate::COMPLETION_CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        let mut query = match receive.recv_timeout(Duration::from_millis(20)) {
            Ok(query) => query,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        while let Ok(newer) = receive.try_recv() {
            query = newer;
        }
        let (ctx, items) = engine::complete(&shell, &worker_snapshot, &query, &names);
        if results.send((query, ctx, items)).is_err() {
            break;
        }
    })));
    let mut ctx = engine::context(&line);
    let mut items = Vec::new();
    let mut selected = 0;
    let mut pending = String::new();
    let mut chosen: Option<(String, Vec<String>)> = None;
    let mut pulse_until = Instant::now();
    let mut dirty = true;
    let mut history_index = memory.history.len();
    let mut draft = line.clone();
    let action = loop {
        if terminated.load(Ordering::Relaxed) {
            line.clear();
            point = 0;
            break "cancel";
        }
        let query = &line[..point];
        if query != pending {
            pending = query.into();
            items.clear();
            selected = 0;
            if !query.is_empty() {
                let _ = send.send(query.into());
            }
            dirty = true;
        }
        while let Ok((query, new_context, mut hits)) = updates.try_recv() {
            if query == line[..point] {
                memory.rank(&new_context, &mut hits);
                if chosen.as_ref().is_some_and(|(at, previous)| {
                    at == &line
                        && *previous == hits.iter().map(|i| i.label.clone()).collect::<Vec<_>>()
                }) {
                    hits.clear();
                }
                ctx = new_context;
                items = hits;
                selected = 0;
                dirty = true;
            }
        }
        let ghost = if point == line.len() {
            memory.suggest(&line)
        } else {
            String::new()
        };
        if dirty {
            screen.draw(
                &line,
                point,
                &ghost,
                &items,
                selected,
                Instant::now() < pulse_until,
            )?;
            dirty = false;
        }
        if !event::poll(Duration::from_millis(15))? {
            if pulse_until.elapsed() < Duration::from_millis(30) {
                dirty = true;
            }
            continue;
        }
        let event = event::read()?;
        match event {
            Event::Resize(_, _) => {}
            Event::Paste(text) => {
                // Terminal paste commonly uses CR; shell editing buffers use LF.
                let text = text.replace("\r\n", "\n").replace('\r', "\n");
                line.insert_str(point, &text);
                point += text.len();
                if line.chars().any(char::is_control) {
                    break "return";
                }
            }
            Event::Key(key) => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key.modifiers.contains(KeyModifiers::ALT);
                match key.code {
                    KeyCode::Enter => {
                        line.push_str(&ghost);
                        point = line.len();
                        if !shell_is_fish {
                            memory.executed(&line);
                        }
                        break "execute";
                    }
                    KeyCode::Tab => break "complete",
                    KeyCode::Esc => break "escape",
                    KeyCode::Char('c') if ctrl => {
                        line.clear();
                        point = 0;
                        break "cancel";
                    }
                    KeyCode::Char('d') if ctrl && line.is_empty() => break "eof",
                    KeyCode::Char('r') if ctrl => break "search",
                    KeyCode::Char('e') if ctrl => {
                        if point == line.len() {
                            line.push_str(&ghost);
                        }
                        point = line.len();
                    }
                    KeyCode::End => point = line.len(),
                    KeyCode::Home | KeyCode::Char('a') if key.code == KeyCode::Home || ctrl => {
                        point = 0
                    }
                    KeyCode::Left | KeyCode::Char('b') if key.code == KeyCode::Left || ctrl => {
                        point = previous(&line, point)
                    }
                    KeyCode::Right | KeyCode::Char('f') if key.code == KeyCode::Right || ctrl => {
                        if point < line.len() {
                            point = next(&line, point);
                        } else if let Some(item) = items.get(selected) {
                            let previous = items.iter().map(|i| i.label.clone()).collect();
                            let prefix = ctx.words.join(" ");
                            memory.record("C", &prefix, &item.label);
                            let insert = format!(
                                "{}{}{}",
                                if ctx.injected { " " } else { "" },
                                engine::quote(&item.label),
                                if item.label.ends_with('/') { "" } else { " " }
                            );
                            line.replace_range(ctx.start..point, &insert);
                            point = ctx.start + insert.len();
                            chosen = Some((line.clone(), previous));
                            // A new context may expose another level; stale rows disappear immediately.

                            items.clear();
                        } else {
                            line.push_str(&ghost);
                            point = line.len();
                        }
                    }
                    KeyCode::Up | KeyCode::Down => {
                        if !items.is_empty() {
                            if key.code == KeyCode::Up {
                                selected = selected.saturating_sub(1);
                            } else {
                                selected = (selected + 1).min(items.len() - 1);
                            }
                            pulse_until = Instant::now() + Duration::from_millis(60);
                        } else {
                            if history_index == memory.history.len() {
                                draft = line.clone();
                            }
                            if key.code == KeyCode::Up {
                                history_index = history_index.saturating_sub(1);
                            } else {
                                history_index = (history_index + 1).min(memory.history.len());
                            }
                            line = memory
                                .history
                                .get(history_index)
                                .cloned()
                                .unwrap_or_else(|| draft.clone());
                            point = line.len();
                        }
                    }
                    KeyCode::Backspace => {
                        if point > 0 {
                            let start = previous(&line, point);
                            line.replace_range(start..point, "");
                            point = start;
                        }
                    }
                    KeyCode::Delete | KeyCode::Char('d') if key.code == KeyCode::Delete || ctrl => {
                        if point < line.len() {
                            line.replace_range(point..next(&line, point), "");
                        }
                    }
                    KeyCode::Char('u') if ctrl => {
                        line.replace_range(..point, "");
                        point = 0;
                    }
                    KeyCode::Char('k') if ctrl => {
                        line.truncate(point);
                    }
                    KeyCode::Char('w') if ctrl => {
                        let end = point;
                        while point > 0 && line[..point].ends_with(char::is_whitespace) {
                            point = previous(&line, point);
                        }
                        while point > 0 && !line[..point].ends_with(char::is_whitespace) {
                            point = previous(&line, point);
                        }
                        line.replace_range(point..end, "");
                    }
                    KeyCode::Char(c) if !ctrl && !alt => {
                        line.insert(point, c);
                        point += c.len_utf8();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        dirty = true;
    };
    drop(send);
    drop(updates);
    drop(worker);
    screen.clear()?;
    // Restore the line the host actually drew; its native redisplay owns the accepted text.
    queue!(screen.tty, MoveTo(screen.x, screen.y))?;
    screen.tty.flush()?;
    fs::write(
        snapshot.join("result"),
        format!(
            "{action}\n{}\n{line}",
            if shell_is_fish {
                line[..point.min(line.len())].chars().count()
            } else {
                point
            }
        ),
    )?;
    let prompt_rows = std::env::var("RFIG_PROMPT_ROWS")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    let host_y = screen.y.saturating_sub(prompt_rows);
    drop(screen);
    if !shell_is_fish {
        let mut tty = io::stdout();
        queue!(tty, MoveTo(0, host_y), Clear(ClearType::FromCursorDown))?;
        tty.flush()?;
    }
    Ok(())
}
