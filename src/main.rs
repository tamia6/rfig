use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::{self, BufWriter, Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{self, Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use unicode_width::UnicodeWidthChar;

const GIT: &[(&str, &str)] = &[
    ("add", "暂存文件"),
    ("branch", "管理分支"),
    ("checkout", "切换分支或还原文件"),
    ("clone", "克隆仓库"),
    ("commit", "提交更改"),
    ("diff", "查看差异"),
    ("fetch", "获取远端更新"),
    ("init", "初始化仓库"),
    ("log", "查看提交历史"),
    ("merge", "合并分支"),
    ("pull", "拉取并合并"),
    ("push", "推送提交"),
    ("rebase", "变基"),
    ("remote", "管理远端"),
    ("restore", "还原文件"),
    ("show", "查看对象"),
    ("stash", "暂存工作区"),
    ("status", "查看状态"),
    ("switch", "切换分支"),
    ("tag", "管理标签"),
];

#[derive(Clone, Debug)]
struct Candidate {
    label: String,
    description: String,
    insert: String,
    start: usize,
}

fn byte_cursor(line: &str, chars: usize) -> Option<usize> {
    if chars == line.chars().count() {
        return Some(line.len());
    }
    line.char_indices().nth(chars).map(|(i, _)| i)
}

fn escape_path_segment(name: &str) -> String {
    let mut escaped = String::new();
    for ch in name.chars() {
        if !ch.is_alphanumeric() && !matches!(ch, '.' | '_' | '-') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

fn initialize_catalog() -> io::Result<()> {
    let mut names = Vec::new();
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if name.chars().any(char::is_control) {
                continue;
            }
            if entry
                .metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
            {
                names.push(name);
            }
        }
    }
    names.sort_unstable();
    names.dedup();
    let definitions = Command::new("/bin/zsh")
        .args([
            "-fc",
            "autoload -Uz compinit; compinit -D; print -rl -- ${(k)_comps}",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();
    let defined: HashSet<_> = definitions.lines().collect();
    let supported: Vec<_> = names
        .iter()
        .filter(|name| defined.contains(name.as_str()))
        .cloned()
        .collect();
    let home = env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    let directory = PathBuf::from(home).join(".config/rfig");
    fs::create_dir_all(&directory)?;
    fs::write(directory.join("commands.txt"), names.join("\n") + "\n")?;
    fs::write(directory.join("supported.txt"), supported.join("\n") + "\n")
}

fn setup() -> io::Result<()> {
    let shell = env::var_os("SHELL").unwrap_or_default();
    if Path::new(&shell)
        .file_name()
        .is_none_or(|name| name != "zsh")
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "rfig setup currently requires zsh as the default shell",
        ));
    }
    let home = PathBuf::from(
        env::var_os("HOME")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?,
    );
    let local_script = home.join(".config/rfig/rfig.zsh");
    let brew_prefix = Command::new("brew")
        .args(["--prefix", "rfig"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()));
    let script =
        if env::current_exe()?.starts_with(home.join(".local/bin")) && local_script.is_file() {
            local_script.clone()
        } else {
            brew_prefix
                .map(|prefix| prefix.join("share/rfig/rfig.zsh"))
                .unwrap_or_else(|| local_script.clone())
        };
    if !script.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("shell integration not found: {}", script.display()),
        ));
    }
    initialize_catalog()?;
    let source = format!(
        "source \"{}\"",
        script
            .display()
            .to_string()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('`', "\\`")
    );
    let rc = PathBuf::from(env::var_os("ZDOTDIR").unwrap_or_else(|| home.clone().into_os_string()))
        .join(".zshrc");
    let existing = fs::read_to_string(&rc).unwrap_or_default();
    let old_source = "source \"$HOME/.config/rfig/rfig.zsh\"";
    if !existing
        .lines()
        .any(|line| line == source || (script == local_script && line == old_source))
    {
        let mut file = fs::OpenOptions::new().create(true).append(true).open(&rc)?;
        if !existing.is_empty() && !existing.ends_with('\n') {
            writeln!(file)?;
        }
        writeln!(file, "{source}")?;
    }
    Command::new(env::current_exe()?)
        .arg("analyze")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    println!("rfig is ready. Open a new zsh terminal to use autocomplete.");
    Ok(())
}

fn parse_help_options(help: &str) -> HashMap<String, &'static str> {
    let mut kinds = HashMap::new();
    for line in help.lines() {
        let spec = line.trim_start().split("  ").next().unwrap_or("");
        if !spec.starts_with('-') {
            continue;
        }
        for chunk in spec.split(',') {
            let chunk = chunk.trim_start();
            let end = chunk
                .find(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_')
                .unwrap_or(chunk.len());
            let name = &chunk[..end];
            if !name.starts_with('-') || name == "-" || name == "--" {
                continue;
            }
            let tail = chunk[end..].trim_start();
            let kind = if let Some(value) = tail.strip_prefix('=') {
                let value = value
                    .split(':')
                    .next()
                    .unwrap_or(value)
                    .trim_matches(|ch| ch == '\'' || ch == '"');
                if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
                    "flag"
                } else {
                    "option"
                }
            } else if tail.starts_with("[=")
                || tail.split_whitespace().next().is_some_and(|arg| {
                    (arg.starts_with('<') && arg.ends_with('>'))
                        || (arg.len() > 1
                            && arg.chars().any(char::is_alphabetic)
                            && arg.chars().all(|ch| ch.is_ascii_uppercase() || ch == '_'))
                        || matches!(
                            arg,
                            "string"
                                | "strings"
                                | "int"
                                | "integer"
                                | "number"
                                | "float"
                                | "duration"
                                | "path"
                                | "file"
                                | "value"
                                | "name"
                                | "port"
                        )
                })
            {
                "option"
            } else if tail.is_empty() || tail.starts_with(':') {
                "flag"
            } else {
                continue;
            };
            kinds.insert(name.to_owned(), kind);
        }
    }
    kinds
}

fn help_output(name: &str, args: &[&str], scratch: &Path) -> io::Result<String> {
    let output = fs::File::create(scratch)?;
    let mut child = Command::new(name)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone()?))
        .stderr(Stdio::from(output))
        .spawn()?;
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "help timed out"));
        }
        thread::sleep(Duration::from_millis(10));
    }
    let mut bytes = Vec::new();
    fs::File::open(scratch)?
        .take(256 * 1024)
        .read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn analyze_command(name: &str, directory: &Path) -> io::Result<usize> {
    if name.contains('/') || name.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid command name",
        ));
    }
    fs::create_dir_all(directory)?;
    let scratch = directory.join(format!(".help-{}", std::process::id()));
    let mut kinds = help_output(name, &["--help"], &scratch)
        .map(|help| parse_help_options(&help))
        .unwrap_or_default();
    if kinds.is_empty() {
        if let Ok(help) = help_output(name, &["-h"], &scratch) {
            kinds = parse_help_options(&help);
        }
    }
    if name == "kubectl" {
        if let Ok(help) = help_output(name, &["options"], &scratch) {
            kinds.extend(parse_help_options(&help));
        }
    }
    let _ = fs::remove_file(&scratch);
    let mut entries: Vec<_> = kinds.into_iter().collect();
    entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let result = entries
        .iter()
        .map(|(option, kind)| format!("{option}\t{kind}\n"))
        .collect::<String>();
    let temporary = directory.join(format!(".{name}-{}", std::process::id()));
    fs::write(&temporary, result)?;
    fs::rename(temporary, directory.join(format!("{name}.tsv")))?;
    Ok(entries.len())
}

fn analyze_catalog(name: Option<&str>) -> io::Result<()> {
    let home = env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    let config = PathBuf::from(home).join(".config/rfig");
    let directory = config.join("options");
    if let Some(name) = name {
        println!(
            "{name}: {} options analyzed",
            analyze_command(name, &directory)?
        );
        return Ok(());
    }
    let supported = fs::read_to_string(config.join("supported.txt"))?;
    let mut names: Vec<_> = supported.lines().collect();
    names.sort_unstable_by_key(|name| {
        (
            !matches!(*name, "kubectl" | "brew" | "docker" | "git"),
            *name,
        )
    });
    for name in names {
        let _ = analyze_command(name, &directory);
    }
    Ok(())
}

fn complete(line: &str, cursor: usize) -> Option<Vec<Candidate>> {
    let end = byte_cursor(line, cursor)?;
    let before = &line[..end];
    if before == "git" || before.starts_with("git ") {
        let (start, prefix) = if before == "git" {
            (0, "")
        } else {
            let word = &before[4..];
            if word.contains(char::is_whitespace) {
                return None;
            }
            (4, word)
        };
        return Some(
            GIT.iter()
                .filter(|(name, _)| name.starts_with(prefix))
                .map(|(name, description)| Candidate {
                    label: (*name).into(),
                    description: (*description).into(),
                    insert: if start == 0 {
                        format!("git {name}")
                    } else {
                        (*name).into()
                    },
                    start,
                })
                .collect(),
        );
    }
    if before == "cd" || before.starts_with("cd ") {
        let (start, typed) = if before == "cd" {
            (0, "")
        } else {
            (3, &before[3..])
        };
        if typed.contains(char::is_whitespace) && !typed.contains("\\ ") {
            return None;
        }
        let path = typed.replace("\\ ", " ");
        let split = path.rfind('/').map_or(0, |i| i + 1);
        let (parent, prefix) = path.split_at(split);
        let raw_parent = typed.rfind('/').map_or("", |i| &typed[..=i]);
        let dir = if parent.starts_with('~') {
            PathBuf::from(env::var_os("HOME")?)
                .join(parent.trim_start_matches('~').trim_start_matches('/'))
        } else if parent.is_empty() {
            env::current_dir().ok()?
        } else {
            PathBuf::from(parent)
        };
        let entries = fs::read_dir(dir).ok()?;
        let mut result = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.chars().any(char::is_control) {
                continue;
            }
            if !name.starts_with(prefix) || (!prefix.starts_with('.') && name.starts_with('.')) {
                continue;
            }
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let escaped = escape_path_segment(&name);
            let insert = if start == 0 {
                format!("cd {raw_parent}{escaped}/")
            } else {
                format!("{raw_parent}{escaped}/")
            };
            result.push(Candidate {
                label: format!("{name}/"),
                description: "目录".into(),
                insert,
                start,
            });
        }
        result.sort_by(|a, b| a.label.cmp(&b.label));
        return Some(result);
    }
    None
}

fn native_candidates(line: &str, start_chars: usize, records: &str) -> Vec<Candidate> {
    let Some(start) = byte_cursor(line, start_chars) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for record in records.lines() {
        let mut fields = record.splitn(3, '\t');
        let (Some(label), Some(insert)) = (fields.next(), fields.next()) else {
            continue;
        };
        if label.is_empty()
            || label.chars().any(char::is_control)
            || insert.chars().any(char::is_control)
        {
            continue;
        }
        if !seen.insert(label) {
            continue;
        }
        let description = fields
            .next()
            .filter(|value| !value.is_empty() && !value.chars().any(char::is_control))
            .map(str::to_owned)
            .or_else(|| {
                if line.starts_with("git ") && !line[4..].contains(' ') {
                    GIT.iter()
                        .find(|(name, _)| *name == label)
                        .map(|(_, desc)| (*desc).into())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                if line.starts_with("git checkout ") {
                    "分支或文件".into()
                } else {
                    "补全候选".into()
                }
            });
        candidates.push(Candidate {
            label: label.into(),
            description,
            insert: insert.into(),
            start,
        });
    }
    candidates
}

fn apply(line: &str, cursor: usize, candidate: &Candidate) -> (String, usize) {
    let end = byte_cursor(line, cursor).expect("validated cursor");
    let suffix = &line[end..];
    let space = if suffix.is_empty() && !candidate.insert.ends_with('/') {
        " "
    } else {
        ""
    };
    let updated = format!(
        "{}{}{}{}",
        &line[..candidate.start],
        candidate.insert,
        space,
        suffix
    );
    let new_cursor = updated[..candidate.start + candidate.insert.len() + space.len()]
        .chars()
        .count();
    (updated, new_cursor)
}

struct RawMode;
impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn padded(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let size = ch.width().unwrap_or(0);
        if used + size > width {
            break;
        }
        out.push(ch);
        used += size;
    }
    out.push_str(&" ".repeat(width - used));
    out
}

struct Popup {
    tty: BufWriter<fs::File>,
    x: u16,
    y: u16,
    prompt_x: u16,
    prompt_y: u16,
    width: u16,
    rows: u16,
}

impl Popup {
    fn draw(&mut self, candidates: &[&Candidate], selected: usize) -> io::Result<()> {
        let width = self.width as usize;
        let visible_rows = self.rows.saturating_sub(1) as usize;
        let first = selected.saturating_sub(visible_rows.saturating_sub(1));
        queue!(self.tty, Hide)?;
        for row in 0..self.rows {
            queue!(
                self.tty,
                MoveTo(self.x, self.y + row),
                ResetColor,
                Print(" ".repeat(width))
            )?;
        }
        for (row, item) in candidates.iter().skip(first).take(visible_rows).enumerate() {
            let active = first + row == selected;
            let icon = if item.label.ends_with('/') { '/' } else { '$' };
            queue!(
                self.tty,
                MoveTo(self.x, self.y + row as u16),
                ResetColor,
                SetAttribute(if active {
                    Attribute::Reverse
                } else {
                    Attribute::NoReverse
                }),
                Print(padded("", width))
            )?;
            queue!(
                self.tty,
                MoveTo(self.x, self.y + row as u16),
                SetForegroundColor(if icon == '/' {
                    Color::Green
                } else {
                    Color::Magenta
                }),
                Print(icon),
                SetForegroundColor(Color::Reset),
                Print(" "),
                Print(padded(&item.label, width.saturating_sub(2))),
                SetAttribute(Attribute::NoReverse)
            )?;
        }
        let description = candidates
            .get(selected)
            .map_or("无匹配 · Esc 取消", |item| item.description.as_str());
        queue!(
            self.tty,
            MoveTo(self.x, self.y + self.rows - 1),
            ResetColor,
            SetAttribute(Attribute::Dim),
            Print(padded(&format!(" {description}"), width)),
            SetAttribute(Attribute::NormalIntensity),
            ResetColor
        )?;
        self.tty.flush()
    }

    fn clear(&mut self) -> io::Result<()> {
        for row in 0..self.rows {
            queue!(
                self.tty,
                MoveTo(self.x, self.y + row),
                ResetColor,
                Print(" ".repeat(self.width as usize))
            )?;
        }
        queue!(
            self.tty,
            MoveTo(self.prompt_x, self.prompt_y),
            Show,
            ResetColor
        )?;
        self.tty.flush()
    }
}

fn pick(candidates: Vec<Candidate>, anchor_x: u16) -> io::Result<(Option<Candidate>, bool)> {
    let (_, cursor_y) = cursor::position()?;
    let (columns, terminal_rows) = crossterm::terminal::size()?;
    if columns < 12 || terminal_rows < 4 {
        return Ok((None, false));
    }
    let width = columns.min(56);
    let x = anchor_x.min(columns - width);
    let rows = (candidates.len().min(6) as u16 + 1).min(terminal_rows - 1);
    let scroll = (cursor_y + rows + 1).saturating_sub(terminal_rows);
    let mut tty = BufWriter::new(fs::OpenOptions::new().write(true).open("/dev/tty")?);
    if scroll > 0 {
        queue!(
            tty,
            MoveTo(0, terminal_rows - 1),
            Print("\n".repeat(scroll as usize))
        )?;
        tty.flush()?;
    }
    let prompt_y = cursor_y.saturating_sub(scroll);
    let mut popup = Popup {
        tty,
        x,
        y: prompt_y + 1,
        prompt_x: 0,
        prompt_y,
        width,
        rows,
    };
    enable_raw_mode()?;
    let _raw = RawMode;
    let result = (|| -> io::Result<(Option<Candidate>, bool)> {
        let mut selected = 0usize;
        let mut filter = String::new();
        loop {
            let visible: Vec<_> = candidates
                .iter()
                .filter(|item| item.label.starts_with(&filter))
                .collect();
            if selected >= visible.len() {
                selected = 0;
            }
            popup.draw(&visible, selected)?;
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Esc => return Ok((None, false)),
                    KeyCode::Char('c')
                        if key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        return Ok((None, false))
                    }
                    KeyCode::Up => selected = selected.saturating_sub(1),
                    KeyCode::Down => {
                        if !visible.is_empty() {
                            selected = (selected + 1).min(visible.len() - 1);
                        }
                    }
                    KeyCode::Tab => {
                        return Ok((visible.get(selected).map(|item| (*item).clone()), true))
                    }
                    KeyCode::Enter => {
                        return Ok((visible.get(selected).map(|item| (*item).clone()), false))
                    }
                    KeyCode::Backspace => {
                        filter.pop();
                        selected = 0;
                    }
                    KeyCode::Char(ch) => {
                        filter.push(ch);
                        selected = 0;
                    }
                    _ => {}
                }
            }
        }
    })();
    let _ = popup.clear();
    result
}

fn run() -> io::Result<i32> {
    let mut args = env::args();
    let _ = args.next();
    match args.next().as_deref() {
        Some("setup") => {
            setup()?;
            return Ok(0);
        }
        Some("init") => {
            initialize_catalog()?;
            return Ok(0);
        }
        Some("analyze") => {
            analyze_catalog(args.next().as_deref())?;
            return Ok(0);
        }
        Some("position") => {
            let (x, _) = cursor::position()?;
            write!(io::stderr(), "{x}")?;
            return Ok(0);
        }
        Some("pick") => {}
        _ => {
            eprintln!("usage: rfig <setup|init|analyze|pick>");
            return Ok(2);
        }
    }
    let cursor = match args.next().and_then(|value| value.parse::<usize>().ok()) {
        Some(value) => value,
        None => return Ok(2),
    };
    let anchor_x = match args.next().and_then(|value| value.parse::<u16>().ok()) {
        Some(value) => value,
        None => return Ok(2),
    };
    let start_chars = match args.next().and_then(|value| value.parse::<usize>().ok()) {
        Some(value) => value,
        None => return Ok(2),
    };
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let (line, records) = input.split_once('\n').unwrap_or((&input, ""));
    if line.contains('\n') || line.contains('\r') {
        return Ok(2);
    }
    let mut candidates = native_candidates(line, start_chars, records);
    if candidates.is_empty() {
        candidates = complete(line, cursor).unwrap_or_default();
    }
    if candidates.is_empty() {
        return Ok(2);
    }
    if let (Some(choice), continue_next) = pick(candidates, anchor_x)? {
        let (new_line, new_cursor) = apply(line, cursor, &choice);
        // stdout stays attached to the terminal for crossterm's cursor query.
        write!(io::stderr(), "{new_cursor}:{new_line}")?;
        return Ok(if continue_next { 10 } else { 0 });
    }
    Ok(1)
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("rfig: {err}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_marks_value_options_and_boolean_switches() {
        let help = "    --as='': Username to impersonate\n    --cache-dir='/tmp': Default cache directory\n    --disable-compression=false: Disable compression\n    --role <role>  Role\n    --verbose  Print details\n";
        let kinds = parse_help_options(help);
        assert_eq!(kinds.get("--as"), Some(&"option"));
        assert_eq!(kinds.get("--cache-dir"), Some(&"option"));
        assert_eq!(kinds.get("--disable-compression"), Some(&"flag"));
        assert_eq!(kinds.get("--role"), Some(&"option"));
        assert_eq!(kinds.get("--verbose"), Some(&"flag"));
    }

    #[test]
    fn git_tab_replaces_prefix_and_keeps_suffix() {
        let items = complete("git c", 5).unwrap();
        let checkout = items.iter().find(|x| x.label == "checkout").unwrap();
        assert_eq!(
            apply("git c --help", 5, checkout),
            ("git checkout --help".into(), 12)
        );
    }

    #[test]
    fn unsupported_command_uses_shell_completion() {
        assert!(complete("cargo ", 6).is_none());
    }

    #[test]
    fn git_without_space_opens_subcommands() {
        let items = complete("git", 3).unwrap();
        let status = items.iter().find(|x| x.label == "status").unwrap();
        assert_eq!(apply("git", 3, status), ("git status ".into(), 11));
    }

    #[test]
    fn filesystem_name_is_safe_in_shell() {
        assert_eq!(
            escape_path_segment("a b$(touch hacked)"),
            "a\\ b\\$\\(touch\\ hacked\\)"
        );
    }

    #[test]
    fn cd_completes_existing_directory() {
        let line = format!("cd {}/s", env!("CARGO_MANIFEST_DIR"));
        let items = complete(&line, line.chars().count()).unwrap();
        let src = items.iter().find(|item| item.label == "src/").unwrap();
        assert_eq!(
            apply(&line, line.chars().count(), src).0,
            format!("cd {}/src/", env!("CARGO_MANIFEST_DIR"))
        );
    }

    #[test]
    fn native_completion_extends_a_nested_command() {
        let line = "git checkout ";
        let items = native_candidates(line, line.chars().count(), "feature\tfeature\tbranch\n");
        assert_eq!(
            apply(line, line.chars().count(), &items[0]).0,
            "git checkout feature "
        );
        assert_eq!(items[0].description, "branch");
    }
}
