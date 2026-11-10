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
#[cfg(unix)]
struct TerminationSignals(Vec<signal_hook::SigId>);
#[cfg(unix)]
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

// Count physical prompt rows, including wrapping and common terminal cursor controls.
fn prompt_position(prompt: &str, cols: u16) -> (u16, u16) {
    let cols = cols.max(1) as usize;
    let (mut x, mut y) = (0usize, 0usize);
    let mut saved = (0usize, 0usize);
    let mut chars = prompt.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\x1b' => match chars.next() {
                Some('[') => {
                    let mut parameters = String::new();
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            let n = parameters
                                .split(';')
                                .next()
                                .and_then(|v| v.parse::<usize>().ok())
                                .unwrap_or(1)
                                .max(1);
                            match c {
                                's' => saved = (x, y),
                                'u' => {
                                    (x, y) = saved;
                                }
                                'G' | '`' => x = (n - 1).min(cols - 1),
                                'C' => x = (x + n).min(cols - 1),
                                'D' => x = x.saturating_sub(n),
                                'A' => y = y.saturating_sub(n),
                                'B' => y += n,
                                'E' => {
                                    y += n;
                                    x = 0;
                                }
                                'F' => {
                                    y = y.saturating_sub(n);
                                    x = 0;
                                }
                                _ => {}
                            }
                            break;
                        }
                        parameters.push(c);
                    }
                }
                Some('7') => saved = (x, y),
                Some('8') => {
                    (x, y) = saved;
                }
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\x07' || (c == '\x1b' && chars.next() == Some('\\')) {
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\n' => {
                y += 1;
                x = 0;
            }
            '\r' => x = 0,
            '\t' => x = ((x / 8 + 1) * 8).min(cols),
            c if !c.is_control() => {
                let width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                if width > 0 && x + width > cols {
                    y += 1;
                    x = 0;
                }
                x += width;
            }
            _ => {}
        }
    }
    if x == cols {
        y += 1;
        x = 0;
    }
    (x as u16, y.min(u16::MAX as usize) as u16)
}

fn input_position(text: &str, x: u16, cols: u16) -> (u16, u16) {
    if text.contains(['\n', '\t']) {
        return prompt_position(&format!("{}{}", " ".repeat(x as usize), text), cols);
    }
    let pos = x as usize + UnicodeWidthStr::width(text);
    (
        (pos % cols as usize) as u16,
        (pos / cols as usize).min(u16::MAX as usize) as u16,
    )
}

struct Screen {
    tty: crate::platform::TerminalOutput,
    x: u16,
    y: u16,
    cols: u16,
    rows: u16,
    drawn: u16,
    prompt: String,
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
        items: &[Item],
        selected: usize,
        ctx: &engine::Context,
    ) -> io::Result<()> {
        self.clear()?;
        let (cols, rows) = terminal::size()?;
        self.cols = cols.max(1);
        self.rows = rows.max(1);
        self.x = self.x.min(self.cols - 1);
        self.y = self.y.min(self.rows - 1);
        let suggestion = items
            .get(selected)
            .and_then(|item| preview_suffix(item, ctx, line, point))
            .unwrap_or_default();
        let rendered = format!("{line}{suggestion}");
        let line_rows = input_position(&rendered, self.x, self.cols).1 as usize + 1;
        let first = selected.saturating_sub(4);
        let menu_rows = items.len().saturating_sub(first).min(5) + usize::from(!items.is_empty());
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
        queue!(self.tty, MoveTo(self.x, self.y))?;
        if line.contains('\n') {
            queue!(self.tty, Print(line.replace('\n', "\r\n")))?;
        } else {
            queue!(self.tty, Print(line))?;
        }
        if !suggestion.is_empty() {
            queue!(
                self.tty,
                SetForegroundColor(Color::DarkGrey),
                Print(suggestion),
                ResetColor
            )?;
        }
        if !items.is_empty() {
            let header_y = self.y.saturating_add(line_rows as u16);
            if header_y < self.rows {
                queue!(
                    self.tty,
                    MoveTo(0, header_y),
                    SetForegroundColor(Color::DarkGrey),
                    Print(crate::padded(
                        &format!(" {}/{}", selected + 1, items.len()),
                        (self.cols as usize).saturating_sub(1)
                    )),
                    ResetColor
                )?;
            }
            let width = (self.cols as usize).saturating_sub(1);
            let label_width = if width > 52 { 36 } else { width / 2 };
            for (visible, (offset, item)) in
                items.iter().enumerate().skip(first).take(5).enumerate()
            {
                let y = self.y.saturating_add((line_rows + 1 + visible) as u16);
                if y >= self.rows {
                    break;
                }
                let (icon, color) = match item.kind.as_str() {
                    "history" => ("◷", Color::DarkGrey),
                    "command" => ("⌘", Color::DarkCyan),
                    "subcommand" => ("↳", Color::DarkMagenta),
                    "option" => ("◇", Color::DarkBlue),
                    "flag" => ("⚑", Color::DarkYellow),
                    _ => ("●", Color::DarkGreen),
                };
                let pointer = if offset == selected { "›" } else { " " };
                let row = format!(
                    "{pointer}{} {icon} {} {}",
                    visible + 1,
                    crate::padded(&item.label, label_width),
                    item.description
                );
                queue!(
                    self.tty,
                    MoveTo(0, y),
                    SetForegroundColor(color),
                    SetAttribute(if offset == selected {
                        Attribute::Bold
                    } else {
                        Attribute::NormalIntensity
                    }),
                    Print(crate::padded(&row, width)),
                    ResetColor,
                    SetAttribute(Attribute::Reset)
                )?;
            }
        }
        self.drawn = height;
        let (cursor_x, cursor_y) = input_position(&line[..point], self.x, self.cols);
        queue!(
            self.tty,
            MoveTo(cursor_x, self.y.saturating_add(cursor_y).min(self.rows - 1)),
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

fn is_input_key(key: &event::KeyEvent) -> bool {
    key.kind != event::KeyEventKind::Release
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

fn preview_suffix(item: &Item, ctx: &engine::Context, line: &str, point: usize) -> Option<String> {
    if point != line.len() {
        return None;
    }
    let mut preview = line.to_owned();
    let mut preview_point = point;
    insert_item(item, ctx, &mut preview, &mut preview_point);
    preview
        .starts_with(line)
        .then(|| preview[point..].to_owned())
}

fn insert_item(item: &Item, ctx: &engine::Context, line: &mut String, point: &mut usize) -> bool {
    if item.kind == "history" {
        *line = item.label.clone();
        *point = line.len();
        return false;
    }
    let insert = format!(
        "{}{}{}",
        if ctx.injected { " " } else { "" },
        item.insertion
            .clone()
            .unwrap_or_else(|| engine::quote_for_shell(
                &item.label,
                &line[ctx.start..*point],
                &ctx.shell
            )),
        if item
            .insertion
            .as_deref()
            .unwrap_or(&item.label)
            .ends_with(['/', '\\'])
            || *point < line.len()
        {
            ""
        } else {
            " "
        }
    );
    line.replace_range(ctx.start..*point, &insert);
    *point = ctx.start + insert.len();
    true
}

pub fn run(
    shell: String,
    snapshot: PathBuf,
    mut line: String,
    mut point: usize,
    displayed: usize,
) -> io::Result<()> {
    if !matches!(shell.as_str(), "bash" | "fish" | "zsh" | "powershell")
        || !line.is_char_boundary(point)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid editor arguments",
        ));
    }
    if shell != "powershell" && line.chars().any(char::is_control) {
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
    let original_line = line.clone();
    let original_point = point;
    let terminated = Arc::new(AtomicBool::new(false));
    #[cfg(unix)]
    let mut signals = TerminationSignals(Vec::new());
    #[cfg(unix)]
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGHUP] {
        signals
            .0
            .push(signal_hook::flag::register(signal, terminated.clone())?);
    }
    terminal::enable_raw_mode()?;
    let _raw = crate::RawMode;
    let mut tty = crate::platform::terminal_output()?;
    queue!(tty, EnableBracketedPaste)?;
    tty.flush()?;
    let saved_anchor = if shell == "powershell" {
        std::env::var("RFIG_ANCHOR_X")
            .ok()
            .and_then(|v| v.parse().ok())
            .zip(
                std::env::var("RFIG_ANCHOR_Y")
                    .ok()
                    .and_then(|v| v.parse().ok()),
            )
    } else {
        None
    };
    let (mut x, mut y) = match saved_anchor {
        Some(anchor) => anchor,
        None => cursor::position()?,
    };
    let (cols, rows) = terminal::size()?;
    let prompt = std::env::var("RFIG_PROMPT").unwrap_or_default();
    let shown = &line[..displayed.min(line.len())];
    let width = UnicodeWidthStr::width(shown) as u16;
    if shell == "bash" {
        // bind -x clears the current physical line. Repaint the frozen prompt from
        // the live cursor, never from DECSC's saved screen coordinates.
        let (prompt_x, prompt_rows) = prompt_position(&prompt, cols);
        let input_rows = (prompt_x as usize + width as usize) / cols.max(1) as usize;
        let top = y
            .saturating_sub(prompt_rows)
            .saturating_sub(input_rows as u16);
        queue!(
            tty,
            MoveTo(0, top),
            Clear(ClearType::FromCursorDown),
            Print(&prompt)
        )?;
        tty.flush()?;
        (x, y) = cursor::position()?;
    }
    let offset = if shell == "bash" {
        x as usize
    } else {
        (x as usize + cols as usize - (width as usize % cols as usize)) % cols as usize
    };
    let prior_rows = if shell == "bash" {
        0
    } else {
        (offset + width as usize) / cols as usize
    };
    let mut screen = Screen {
        tty,
        x: offset as u16,
        y: y.saturating_sub(prior_rows as u16),
        cols,
        rows,
        drawn: 1,
        prompt,
    };
    let mut initial_key =
        std::env::var("RFIG_INITIAL_KEY")
            .ok()
            .and_then(|key| match key.as_str() {
                "\t" => Some(event::KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
                "\x1b[1;3A" | "\x1b\x1b[A" => {
                    Some(event::KeyEvent::new(KeyCode::Up, KeyModifiers::ALT))
                }
                "\x1b[1;3B" | "\x1b\x1b[B" => {
                    Some(event::KeyEvent::new(KeyCode::Down, KeyModifiers::ALT))
                }
                "\x1b[1;3C" | "\x1b\x1b[C" => {
                    Some(event::KeyEvent::new(KeyCode::Right, KeyModifiers::ALT))
                }
                "\x1b[1;3D" | "\x1b\x1b[D" => {
                    Some(event::KeyEvent::new(KeyCode::Left, KeyModifiers::ALT))
                }
                _ => None,
            });
    let mut memory = Memory::load(
        engine::config(),
        std::env::var("PWD").unwrap_or_else(|_| {
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        }),
    );
    let names = if matches!(shell.as_str(), "bash" | "fish") {
        engine::names(&snapshot)
    } else {
        Vec::new()
    };
    let (send, receive) = mpsc::channel::<String>();
    let (results, updates) = mpsc::channel();
    let worker_snapshot = snapshot.clone();
    let shell_is_fish = shell == "fish";
    let shell_is_powershell = shell == "powershell";
    let shell_is_bash = shell == "bash";
    let shell_is_zsh = shell == "zsh";
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
    let mut focused_label: Option<String> = None;
    let mut pending = String::new();
    let mut completed = String::new();
    let mut chosen: Option<(String, Vec<String>)> = None;
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
            completed.clear();
            items.clear();
            focused_label = None;
            if point == line.len() {
                memory.add_history(&line, &mut items);
            }
            selected = 0;
            if !query.is_empty() {
                let _ = send.send(query.into());
            }
            dirty = true;
        }
        while let Ok((query, new_context, mut hits)) = updates.try_recv() {
            if query == line[..point] {
                completed = query.clone();
                memory.rank(&new_context, &mut hits);
                if chosen.as_ref().is_some_and(|(at, previous)| {
                    at == &line
                        && *previous == hits.iter().map(|i| i.label.clone()).collect::<Vec<_>>()
                }) {
                    hits.clear();
                }
                if point == line.len() {
                    memory.add_history(&line, &mut hits);
                }
                ctx = new_context;
                items = hits;
                selected = focused_label
                    .as_ref()
                    .and_then(|label| items.iter().position(|item| item.label == *label))
                    .unwrap_or(0);
                dirty = true;
            }
        }
        if dirty {
            screen.draw(&line, point, &items, selected, &ctx)?;
            dirty = false;
        }
        let event = if let Some(key) = initial_key.take() {
            Event::Key(key)
        } else {
            if !event::poll(Duration::from_millis(15))? {
                continue;
            }
            crate::input::read()?
        };
        match event {
            Event::Resize(cols, rows) => {
                // The terminal can reflow the input: saved absolute coordinates are stale.
                let new_y = cursor::position().ok().map(|(_, y)| y);
                screen.cols = cols.max(1);
                screen.rows = rows.max(1);
                if !screen.prompt.is_empty() {
                    screen.x = prompt_position(&screen.prompt, screen.cols).0;
                } else {
                    screen.x = screen.x.min(screen.cols - 1);
                }
                let cursor_rows = (screen.x as usize + UnicodeWidthStr::width(&line[..point]))
                    / screen.cols as usize;
                if let Some(y) = new_y {
                    screen.y = y.saturating_sub(cursor_rows as u16);
                }
            }
            Event::Paste(text) => {
                // Terminal paste commonly uses CR; shell editing buffers use LF.
                let mut text = text.replace("\r\n", "\n").replace('\r', "\n");
                if shell_is_powershell {
                    text.retain(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'));
                }
                line.insert_str(point, &text);
                point += text.len();
                if !shell_is_powershell && line.chars().any(char::is_control) {
                    break "return";
                }
            }
            Event::Key(key) => {
                if !is_input_key(&key) {
                    continue;
                }
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key.modifiers.contains(KeyModifiers::ALT);
                let super_key = key.modifiers.contains(KeyModifiers::SUPER);
                if (key.code == KeyCode::Tab
                    || ((alt || super_key)
                        && matches!(
                            key.code,
                            KeyCode::Up
                                | KeyCode::Down
                                | KeyCode::Left
                                | KeyCode::Right
                                | KeyCode::Char('1'..='5')
                        )))
                    && completed != line[..point]
                    && !pending.is_empty()
                {
                    // Explicit selection waits briefly for the current query, including history-only provisional menus.
                    let deadline = Instant::now() + Duration::from_millis(400);
                    while completed != line[..point] && Instant::now() < deadline {
                        let Ok((query, new_context, mut hits)) =
                            updates.recv_timeout(Duration::from_millis(20))
                        else {
                            continue;
                        };
                        if query != line[..point] {
                            continue;
                        }
                        completed = query;
                        memory.rank(&new_context, &mut hits);
                        if point == line.len() {
                            memory.add_history(&line, &mut hits);
                        }
                        ctx = new_context;
                        items = hits;
                        selected = focused_label
                            .as_ref()
                            .and_then(|label| items.iter().position(|item| item.label == *label))
                            .unwrap_or(0);
                    }
                }
                match key.code {
                    KeyCode::Enter | KeyCode::Char('j') if key.code == KeyCode::Enter || ctrl => {
                        point = line.len();
                        if !shell_is_fish {
                            memory.executed(&line);
                        }
                        break "execute";
                    }
                    KeyCode::Tab => {
                        if let Some(item) = items.get(selected) {
                            let previous = items.iter().map(|i| i.label.clone()).collect();
                            if insert_item(item, &ctx, &mut line, &mut point) {
                                memory.record("C", &ctx.words.join(" "), &item.label);
                                chosen = Some((line.clone(), previous));
                            }
                            focused_label = None;
                            items.clear();
                        } else if !shell_is_powershell {
                            break "complete";
                        }
                    }
                    KeyCode::Esc if shell_is_powershell => {
                        line.clear();
                        point = 0;
                        break "cancel";
                    }
                    KeyCode::Esc => break "escape",
                    KeyCode::Char('c') if ctrl => {
                        line.clear();
                        point = 0;
                        break "cancel";
                    }
                    KeyCode::Char('d') if ctrl && line.is_empty() => break "eof",
                    KeyCode::Char('r') if ctrl && shell_is_powershell => {
                        if let Some(found) = memory
                            .history
                            .iter()
                            .rev()
                            .find(|entry| entry.contains(&line))
                        {
                            line = found.clone();
                            point = line.len();
                        }
                    }
                    KeyCode::Char('r') if ctrl => break "search",
                    KeyCode::Char('e') if ctrl => {
                        point = line.len();
                    }
                    KeyCode::End => point = line.len(),
                    KeyCode::Home | KeyCode::Char('a') if key.code == KeyCode::Home || ctrl => {
                        point = 0
                    }
                    KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
                        if alt || super_key =>
                    {
                        if items.is_empty() {
                            continue;
                        }
                        selected = match key.code {
                            KeyCode::Left | KeyCode::Up => selected.saturating_sub(1),
                            _ => (selected + 1).min(items.len() - 1),
                        };
                        focused_label = items.get(selected).map(|item| item.label.clone());
                    }
                    KeyCode::Left | KeyCode::Char('b')
                        if (key.code == KeyCode::Left && !alt && !super_key) || ctrl =>
                    {
                        point = previous(&line, point)
                    }
                    KeyCode::Right | KeyCode::Char('f')
                        if (key.code == KeyCode::Right && !alt && !super_key) || ctrl =>
                    {
                        if point < line.len() {
                            point = next(&line, point);
                        }
                    }
                    KeyCode::Up | KeyCode::Down => {
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
                    KeyCode::Char(digit) if (alt || super_key) && ('1'..='5').contains(&digit) => {
                        if !items.is_empty() {
                            let first = selected.saturating_sub(4);
                            let shortcut = digit.to_digit(10).unwrap_or(1) as usize;
                            selected = (first + shortcut - 1).min(items.len() - 1);
                            focused_label = items.get(selected).map(|item| item.label.clone());
                        }
                    }
                    KeyCode::Char(c) if ctrl && shell_is_zsh => {
                        if c.is_ascii_lowercase() {
                            fs::write(snapshot.join("selection"), (selected + 1).to_string())?;
                            fs::write(snapshot.join("key"), [c as u8 - b'a' + 1])?;
                            break "native";
                        }
                    }
                    KeyCode::Char(c) if !ctrl && !alt && !super_key => {
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
    if action == "execute" {
        // The completion worker may have queued a taller menu after the last
        // draw. Clear every row below the prompt before the host executes.
        screen.drawn = screen.rows.saturating_sub(screen.y);
    }
    screen.clear()?;
    // Restore the line the host actually drew; its native redisplay owns the accepted text.
    queue!(screen.tty, MoveTo(screen.x, screen.y))?;
    screen.tty.flush()?;
    fs::write(
        snapshot.join("result"),
        format!(
            "{action}\n{}\n{line}",
            if shell_is_powershell {
                line[..point.min(line.len())].encode_utf16().count()
            } else if shell_is_fish {
                line[..point.min(line.len())].chars().count()
            } else {
                point
            }
        ),
    )?;
    // Readline redisplays only the last logical prompt line. Keep earlier
    // prompt lines intact instead of erasing their physical wrapped rows.
    let prompt_tail = screen.prompt.rsplit('\n').next().unwrap_or("");
    let prompt_rows = prompt_position(prompt_tail, screen.cols).1;
    let host_y = screen.y.saturating_sub(prompt_rows);
    let host_x = screen.x;
    let input_y = screen.y;
    let host_cols = screen.cols;
    drop(screen);
    if shell_is_zsh {
        // ZLE computes screen differences from the buffer it drew at handoff.
        // Restore that baseline before it redisplays the Rust-edited buffer.
        let mut tty = crate::platform::terminal_output()?;
        let pos = host_x as usize + UnicodeWidthStr::width(&original_line[..original_point]);
        queue!(
            tty,
            MoveTo(host_x, input_y),
            Print(&original_line),
            MoveTo(
                (pos % host_cols as usize) as u16,
                input_y.saturating_add((pos / host_cols as usize) as u16)
            )
        )?;
        tty.flush()?;
    }
    if shell_is_bash {
        let mut tty = io::stdout();
        queue!(tty, MoveTo(0, host_y), Clear(ClearType::FromCursorDown))?;
        tty.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod geometry_tests {
    use super::{preview_suffix, prompt_position};
    use crate::engine::{self, Item};

    #[test]
    fn windows_release_events_do_not_edit_twice() {
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
        for kind in [
            KeyEventKind::Press,
            KeyEventKind::Repeat,
            KeyEventKind::Release,
        ] {
            let key = KeyEvent::new_with_kind(KeyCode::Char('x'), KeyModifiers::NONE, kind);
            assert_eq!(super::is_input_key(&key), kind != KeyEventKind::Release);
        }
    }

    #[test]
    fn physical_prompt_rows_include_wrap_and_terminal_controls() {
        assert_eq!(prompt_position("abcdefghij\n$ ", 8), (2, 2));
        assert_eq!(prompt_position("\x1b[31m中文\x1b[0m$ ", 5), (1, 1));
        assert_eq!(
            prompt_position("user\x1b[s\x1b[45G12:00\x1b[u\n$ ", 58),
            (2, 1)
        );
        assert_eq!(
            prompt_position("\x1b]0;window title\x07user\n$ ", 58),
            (2, 1)
        );
        assert_eq!(prompt_position("12345678\n$ ", 8), (2, 1));
    }

    #[test]
    fn inline_preview_contains_only_the_missing_suffix() {
        let line = "git che";
        let ctx = engine::context(line);
        let item = Item {
            label: "checkout".into(),
            description: String::new(),
            kind: "subcommand".into(),
            popular: 0,
            insertion: None,
        };
        assert_eq!(
            preview_suffix(&item, &ctx, line, line.len()),
            Some("ckout ".into())
        );
        assert_eq!(preview_suffix(&item, &ctx, line, 3), None);
    }
}
