mod editor;
mod engine;
mod input;
mod platform;
mod sandbox;
#[cfg(unix)]
use libc::{flock, setrlimit, setsid};
#[cfg(unix)]
use std::os::{
    fd::AsRawFd,
    unix::{fs::symlink, process::CommandExt},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    env, fs,
    io::{self, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{self, Hide, MoveTo, Show},
    event::{Event, KeyCode, KeyEventKind},
    queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use unicode_width::UnicodeWidthChar;

static COMPLETION_CANCELLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(unix)]
static COMPLETION_CHILD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
fn cancel_completion() {
    use std::sync::atomic::Ordering;
    COMPLETION_CANCELLED.store(true, Ordering::SeqCst);
    #[cfg(unix)]
    let pid = COMPLETION_CHILD.load(Ordering::SeqCst);
    #[cfg(unix)]
    platform::kill_group(pid);
    #[cfg(windows)]
    platform::cancel_job();
}

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

const HELP: &str = "Usage: rfig <command> [arguments]

Inline command completion and history suggestions for zsh, Bash, Fish and PowerShell. Suggestions appear as you type.

Commands:
  setup [--shell SHELL]  Scan commands, install shell integration, and enrich in the background
  init                  Refresh the local command catalog
  analyze <command>     Refresh completion data for one command
  completion <shell>    Print completion definitions (zsh, bash, fish, powershell)
  terminal-keys         Configure Command-key mappings for macOS Terminal.app

Options:
  -h, --help            Show this help

Examples:
  rfig setup
  rfig analyze kubectl
  rfig completion zsh
  rfig terminal-keys
";

const ZSH_COMPLETION: &str = r#"#compdef rfig
_rfig() {
  if (( CURRENT == 2 )); then
    compadd -- setup init analyze completion terminal-keys
  elif (( CURRENT == 3 )) && [[ $words[2] == analyze ]]; then
    local file="$HOME/.config/rfig/commands.txt"
    [[ -r $file ]] || return
    local -a available
    available=(${(f)"$(<"$file")"})
    compadd -- "${available[@]}"
  elif (( CURRENT == 3 )) && [[ $words[2] == completion ]]; then
    compadd -- zsh bash fish powershell
  elif [[ $words[2] == setup ]]; then
    if (( CURRENT == 3 )); then compadd -- --shell; else compadd -- zsh bash fish powershell; fi
  fi
}
compdef _rfig rfig
"#;

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
            if fs::metadata(entry.path())
                .is_ok_and(|meta| platform::is_executable(&entry.path(), meta))
            {
                names.push(platform::command_name(&name));
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
    let home = env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    let home = PathBuf::from(home);
    let mut supported: Vec<_> = names
        .iter()
        .filter(|name| defined.contains(name.as_str()))
        .cloned()
        .collect();
    let xdg_config = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let xdg_data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    let completion_dirs = [
        xdg_config.join("fish/completions"),
        xdg_data.join("fish/vendor_completions.d"),
        xdg_data.join("bash-completion/completions"),
        home.join(".bash_completion.d"),
        PathBuf::from("/opt/homebrew/etc/fish/completions"),
        PathBuf::from("/opt/homebrew/share/fish/vendor_completions.d"),
        PathBuf::from("/opt/homebrew/etc/bash_completion.d"),
        PathBuf::from("/usr/local/etc/fish/completions"),
        PathBuf::from("/usr/local/share/fish/vendor_completions.d"),
        PathBuf::from("/usr/local/etc/bash_completion.d"),
        PathBuf::from("/usr/share/fish/vendor_completions.d"),
        PathBuf::from("/usr/share/bash-completion/completions"),
        PathBuf::from("/etc/bash_completion.d"),
    ];
    for directory in completion_dirs {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Some(file) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let command = file
                .strip_suffix(".fish")
                .or_else(|| file.strip_suffix(".bash"))
                .unwrap_or(&file);
            if names
                .binary_search_by(|name| name.as_str().cmp(command))
                .is_ok()
            {
                supported.push(command.to_owned());
            }
        }
    }
    for directory in [
        home.join(".zsh/completions"),
        PathBuf::from("/opt/homebrew/share/zsh/site-functions"),
        PathBuf::from("/usr/local/share/zsh/site-functions"),
    ] {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Some(file) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(command) = file.strip_prefix('_') else {
                continue;
            };
            if names
                .binary_search_by(|name| name.as_str().cmp(command))
                .is_ok()
            {
                supported.push(command.to_owned());
            }
        }
    }
    supported.sort_unstable();
    supported.dedup();
    let directory = home.join(".config/rfig");
    fs::create_dir_all(&directory)?;
    fs::write(directory.join("commands.txt"), names.join("\n") + "\n")?;
    fs::write(directory.join("supported.txt"), supported.join("\n") + "\n")
}

fn homebrew_integration(shell: &str) -> Option<PathBuf> {
    let executable = fs::canonicalize(env::current_exe().ok()?).ok()?;
    let keg = executable.parent()?.parent()?;
    let cellar = keg.parent()?.parent()?;
    if cellar.file_name()? != "Cellar" {
        return None;
    }
    // opt follows upgrades; a versioned Cellar path would keep loading the old script.
    let opt = cellar.parent()?.join("opt/rfig");
    if fs::canonicalize(opt.join("bin/rfig")).ok()? != executable {
        return None;
    }
    let script = opt.join(format!("share/rfig/rfig.{shell}"));
    script.is_file().then_some(script)
}

fn install_integration(script: &Path, shell: &str, contents: &str) -> io::Result<()> {
    let staged = script.with_extension(format!("{shell}.{}.tmp", std::process::id()));
    let mut created = false;
    let result = (|| {
        if let Some(packaged) = homebrew_integration(shell) {
            #[cfg(unix)]
            symlink(packaged, &staged)?;
            #[cfg(windows)]
            fs::copy(packaged, &staged)?;
            created = true;
        } else {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staged)?;
            created = true;
            file.write_all(contents.as_bytes())?;
        }
        // Replace the link itself when changing install methods, never its package target.
        fs::rename(&staged, script)
    })();
    if created && result.is_err() {
        let _ = fs::remove_file(staged);
    }
    result
}

fn terminal_key_hint() {
    #[cfg(target_os = "macos")]
    if env::var("TERM_PROGRAM").as_deref() == Ok("Apple_Terminal") {
        println!("For Command+arrow selection, run: rfig terminal-keys");
    }
}

fn setup(requested: Option<&str>) -> io::Result<()> {
    let default_shell = env::var("SHELL").unwrap_or_else(|_| {
        if cfg!(windows) {
            "powershell".into()
        } else {
            String::new()
        }
    });
    let shell = requested.unwrap_or_else(|| default_shell.rsplit('/').next().unwrap_or(""));
    if matches!(shell, "powershell" | "pwsh") {
        return platform::setup_powershell();
    }
    let contents = match shell {
        "zsh" => include_str!("../rfig.zsh"),
        "bash" => include_str!("../rfig.bash"),
        "fish" => include_str!("../rfig.fish"),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "choose a supported shell: rfig setup --shell zsh|bash|fish|powershell",
            ))
        }
    };
    let home = PathBuf::from(
        env::var_os("HOME")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?,
    );
    let script = home.join(format!(".config/rfig/rfig.{shell}"));
    fs::create_dir_all(script.parent().unwrap())?;
    install_integration(&script, shell, contents)?;
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
    let rc = match shell {
        "zsh" => {
            PathBuf::from(env::var_os("ZDOTDIR").unwrap_or_else(|| home.clone().into_os_string()))
                .join(".zshrc")
        }
        "bash" => home.join(".bashrc"),
        _ => PathBuf::from(
            env::var_os("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config").into_os_string()),
        )
        .join("fish/conf.d/rfig.fish"),
    };
    fs::create_dir_all(rc.parent().unwrap())?;
    let mut existing = fs::read_to_string(&rc).unwrap_or_default();
    let legacy_suffix = format!("/share/rfig/rfig.{shell}\"");
    if existing
        .lines()
        .any(|line| line.starts_with("source \"") && line.ends_with(&legacy_suffix))
    {
        let migrated = existing
            .lines()
            .map(|line| {
                if line.starts_with("source \"") && line.ends_with(&legacy_suffix) {
                    source.as_str()
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        fs::write(&rc, &migrated)?;
        existing = migrated;
    }
    let bin_directory = home.join(".local/bin");
    // setup can be invoked by the installer with a temporary PATH; persist the user bin path.
    let path_line = if shell == "fish" {
        "fish_add_path --global $HOME/.local/bin"
    } else {
        "export PATH=\"$HOME/.local/bin:$PATH\""
    };
    if bin_directory.is_dir() && !existing.lines().any(|line| line == path_line) {
        let mut file = fs::OpenOptions::new().create(true).append(true).open(&rc)?;
        writeln!(file, "\n{path_line}")?;
    }
    let old_source = format!("source \"$HOME/.config/rfig/rfig.{shell}\"");
    if !existing
        .lines()
        .any(|line| line == source || line == old_source)
    {
        let mut file = fs::OpenOptions::new().create(true).append(true).open(&rc)?;
        if !existing.is_empty() && !existing.ends_with('\n') {
            writeln!(file)?;
        }
        writeln!(file, "{source}")?;
    }
    // Login Bash reads the first existing profile, not .bashrc automatically.
    if shell == "bash" {
        let profile = [".bash_profile", ".bash_login", ".profile"]
            .iter()
            .map(|name| home.join(name))
            .find(|path| path.is_file())
            .unwrap_or_else(|| home.join(".bash_profile"));
        let text = fs::read_to_string(&profile).unwrap_or_default();
        if !text.contains(".bashrc") {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(profile)?;
            writeln!(
                file,
                "\n[ -n \"$BASH_VERSION\" ] && [ -r \"$HOME/.bashrc\" ] && . \"$HOME/.bashrc\""
            )?;
        }
    }
    if let Err(error) = sandbox::check() {
        fs::write(
            home.join(".config/rfig/enrich.status"),
            format!("unavailable\n{error}\n"),
        )?;
        eprintln!("rfig: background help enrichment unavailable: {error}");
        println!("rfig integration is ready; open a new {shell} terminal. Existing completion definitions and history remain available.");
        terminal_key_hint();
        return Ok(());
    }
    let mut worker = Command::new(env::current_exe()?);
    worker
        .arg("enrich-background")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    unsafe {
        worker.pre_exec(|| {
            if setsid() < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    worker.spawn()?;
    println!("rfig is ready. Completion enrichment is running in the background; open a new {shell} terminal.");
    terminal_key_hint();
    Ok(())
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const TERMINAL_KEY_MAP: [(&str, &str, &str); 9] = [
    ("@F700", "__RFIG_CMD_UP__", "\x1b[1;9A"),
    ("@F701", "__RFIG_CMD_DOWN__", "\x1b[1;9B"),
    ("@F702", "__RFIG_CMD_LEFT__", "\x1b[1;9D"),
    ("@F703", "__RFIG_CMD_RIGHT__", "\x1b[1;9C"),
    ("@1", "__RFIG_CMD_1__", "\x1b1"),
    ("@2", "__RFIG_CMD_2__", "\x1b2"),
    ("@3", "__RFIG_CMD_3__", "\x1b3"),
    ("@4", "__RFIG_CMD_4__", "\x1b4"),
    ("@5", "__RFIG_CMD_5__", "\x1b5"),
];

#[cfg(target_os = "macos")]
fn terminal_app_keys() -> io::Result<()> {
    let home = PathBuf::from(
        env::var_os("HOME")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?,
    );
    let config = home.join(".config/rfig");
    fs::create_dir_all(&config)?;
    let temporary = env::temp_dir().join(format!("rfig-terminal-{}.plist", std::process::id()));
    let backup = config.join("terminal-app-backup.plist");

    let exported = Command::new("defaults")
        .args(["export", "com.apple.Terminal"])
        .arg(&temporary)
        .status()?;
    if !exported.success() {
        return Err(io::Error::other(
            "could not export Terminal.app preferences",
        ));
    }
    fs::copy(&temporary, &backup)?;

    let result = (|| {
        let output = Command::new("defaults")
            .args(["read", "com.apple.Terminal", "Startup Window Settings"])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(
                "Terminal.app has no current startup profile",
            ));
        }
        let profile = String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_matches('"')
            .to_owned();
        if profile.is_empty()
            || !profile
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || " _.-".contains(c))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported Terminal.app profile name",
            ));
        }
        let profile_path = profile.replace(' ', "\\ ");
        let root = format!(":Window\\ Settings:{profile_path}:keyMapBoundKeys");
        let print = Command::new("/usr/libexec/PlistBuddy")
            .args(["-c", &format!("Print {root}"), temporary.to_str().unwrap()])
            .output()?;
        if !print.status.success() {
            let add = Command::new("/usr/libexec/PlistBuddy")
                .args([
                    "-c",
                    &format!("Add {root} dict"),
                    temporary.to_str().unwrap(),
                ])
                .status()?;
            if !add.success() {
                return Err(io::Error::other(
                    "could not create Terminal.app key bindings",
                ));
            }
        }
        for (key, placeholder, _) in TERMINAL_KEY_MAP {
            let path = format!("{root}:{key}");
            let _ = Command::new("/usr/libexec/PlistBuddy")
                .args(["-c", &format!("Delete {path}"), temporary.to_str().unwrap()])
                .output();
            let added = Command::new("/usr/libexec/PlistBuddy")
                .args([
                    "-c",
                    &format!("Add {path} string {placeholder}"),
                    temporary.to_str().unwrap(),
                ])
                .status()?;
            if !added.success() {
                return Err(io::Error::other(format!(
                    "could not write Terminal.app key binding {key}"
                )));
            }
        }
        let mut text = fs::read_to_string(&temporary)?;
        for (_, placeholder, sequence) in TERMINAL_KEY_MAP {
            text = text.replace(placeholder, sequence);
        }
        fs::write(&temporary, text)?;
        let lint = Command::new("plutil")
            .args(["-lint", temporary.to_str().unwrap()])
            .output()?;
        if !lint.status.success() {
            return Err(io::Error::other(
                "generated Terminal.app preferences are invalid",
            ));
        }
        let imported = Command::new("defaults")
            .args(["import", "com.apple.Terminal"])
            .arg(&temporary)
            .status()?;
        if !imported.success() {
            return Err(io::Error::other(
                "could not import Terminal.app preferences",
            ));
        }
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    if result.is_err() {
        let _ = Command::new("defaults")
            .args(["import", "com.apple.Terminal"])
            .arg(&backup)
            .status();
    }
    result?;
    println!(
        "Terminal.app Command-key mappings installed. Open a new Terminal window; backup: {}",
        backup.display()
    );
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn terminal_app_keys() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Terminal.app key mapping is available on macOS only",
    ))
}

fn parse_help_options(help: &str) -> HashMap<String, &'static str> {
    let mut kinds = HashMap::new();
    let mut previous: Vec<String> = Vec::new();
    for line in help.lines() {
        let spec = line.trim_start().split("  ").next().unwrap_or("");
        let annotated = if line.contains("[boolean]") {
            Some("flag")
        } else if ["[string]", "[number]", "[array]"]
            .iter()
            .any(|hint| line.contains(hint))
        {
            Some("option")
        } else {
            None
        };
        if !spec.starts_with('-') {
            if line.starts_with(char::is_whitespace) {
                if let Some(kind) = annotated {
                    for name in &previous {
                        kinds.insert(name.clone(), kind);
                    }
                }
            } else {
                previous.clear();
            }
            continue;
        }
        previous.clear();
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
            let kind = if let Some(kind) = annotated {
                kind
            } else if let Some(value) = tail.strip_prefix('=') {
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
            previous.push(name.to_owned());
        }
    }
    kinds
}

fn help_invocation(help: &str) -> Option<&str> {
    let mut usage = false;
    for line in help.lines() {
        let trimmed = line.trim();
        let text = if let Some((heading, rest)) = trimmed.split_once(':') {
            if !heading.eq_ignore_ascii_case("usage") {
                continue;
            }
            usage = true;
            rest.trim()
        } else if trimmed.eq_ignore_ascii_case("usage") {
            usage = true;
            continue;
        } else if usage && line.starts_with(char::is_whitespace) {
            trimmed
        } else {
            continue;
        };
        if let Some(word) = text
            .split_whitespace()
            .next()
            .filter(|word| safe_command_name(word))
        {
            return Some(word);
        }
    }
    None
}

fn advertised_full_help(help: &str) -> Option<Vec<&str>> {
    let invocation = help_invocation(help)?;
    for line in help
        .lines()
        .filter(|line| line.starts_with(char::is_whitespace))
    {
        let words: Vec<_> = line.split_whitespace().collect();
        if words.starts_with(&[invocation, "help", "--all"]) {
            return Some(vec!["help", "--all"]);
        }
    }
    None
}

fn parse_help_subcommands<'a>(name: &str, help: &'a str) -> Vec<&'a str> {
    let mut commands = Vec::new();
    let (root, suffix) = name.split_once(' ').unwrap_or((name, ""));
    let invocation = format!(
        "{}{}{}",
        help_invocation(help).unwrap_or(root),
        if suffix.is_empty() { "" } else { " " },
        suffix
    );
    let mut in_section = false;
    let mut section_indent = None;
    for line in help.lines() {
        let trimmed = line.trim();
        let heading = trimmed.trim_end_matches(':');
        let uppercase_heading = !heading.is_empty()
            && heading.chars().any(|ch| ch.is_ascii_uppercase())
            && heading
                .chars()
                .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_whitespace() || ch == '_');
        if heading.to_ascii_lowercase().contains("command")
            && (trimmed.ends_with(':')
                || uppercase_heading
                || heading.eq_ignore_ascii_case("commands"))
        {
            in_section = true;
            section_indent = None;
            continue;
        }
        let candidate = if in_section {
            if line.is_empty() || !line.starts_with(char::is_whitespace) || trimmed.ends_with(':') {
                in_section = false;
                None
            } else {
                let indent = line.len() - line.trim_start().len();
                if section_indent.is_some_and(|first| indent > first) {
                    continue;
                }
                section_indent.get_or_insert(indent);
                let field = trimmed.split("  ").next().unwrap_or(trimmed);
                let root_prefix = format!("{} ", help_invocation(help).unwrap_or(root));
                if field.starts_with(&root_prefix) && !field.starts_with(&format!("{invocation} "))
                {
                    continue;
                }
                trimmed
                    .strip_prefix(invocation.as_str())
                    .and_then(|rest| rest.strip_prefix(char::is_whitespace))
                    .unwrap_or(trimmed)
                    .split_whitespace()
                    .next()
                    .map(|word| word.trim_end_matches(':'))
            }
        } else {
            trimmed
                .strip_prefix(invocation.as_str())
                .and_then(|rest| rest.strip_prefix(char::is_whitespace))
                .and_then(|rest| rest.split_whitespace().next())
                .filter(|_| line.starts_with(char::is_whitespace))
        };
        if let Some(command) = candidate.filter(|value| {
            value.starts_with(char::is_alphanumeric)
                && value
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        }) {
            if !commands.contains(&command) {
                commands.push(command);
            }
        }
    }
    commands
}

fn help_advertises_option_command(name: &str, help: &str) -> bool {
    let invocation = format!("\"{name} options\"");
    help.lines()
        .any(|line| line.trim_start().starts_with("Use ") && line.contains(&invocation))
}

fn help_output(
    name: &str,
    args: &[&str],
    scratch: &Path,
    background_safe: bool,
) -> io::Result<String> {
    help_output_status(name, args, scratch, background_safe).map(|(output, _)| output)
}

fn help_output_status(
    name: &str,
    args: &[&str],
    scratch: &Path,
    background_safe: bool,
) -> io::Result<(String, bool)> {
    if COMPLETION_CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "completion cancelled",
        ));
    }
    let output = fs::File::create(scratch)?;
    let mut command = if background_safe {
        sandbox::command(name)
    } else {
        Command::new(name)
    };
    if background_safe {
        command.current_dir(scratch.parent().unwrap());
    }
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone()?))
        // Help may be printed on stderr; completion protocols use stdout for records.
        .stderr(if background_safe {
            Stdio::from(output)
        } else {
            Stdio::null()
        });
    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            let file = libc::rlimit {
                rlim_cur: 512 * 1024,
                rlim_max: 512 * 1024,
            };
            let cpu = libc::rlimit {
                rlim_cur: 2,
                rlim_max: 2,
            };
            if setsid() < 0
                || setrlimit(libc::RLIMIT_FSIZE, &file) < 0
                || setrlimit(libc::RLIMIT_CPU, &cpu) < 0
            {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    if background_safe {
        sandbox::protect(&mut command)?;
    }
    let mut child = platform::spawn_completion(&mut command)?;
    #[cfg(unix)]
    COMPLETION_CHILD.store(child.id() as i32, std::sync::atomic::Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_millis(500);
    let success = loop {
        if COMPLETION_CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
            platform::kill_completion(&mut child);
            let _ = child.wait();
            #[cfg(unix)]
            COMPLETION_CHILD.store(0, std::sync::atomic::Ordering::SeqCst);
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "completion cancelled",
            ));
        }
        if let Some(status) = child.try_wait()? {
            platform::kill_completion(&mut child);
            #[cfg(unix)]
            COMPLETION_CHILD.store(0, std::sync::atomic::Ordering::SeqCst);
            break status.success();
        }
        if Instant::now() >= deadline {
            platform::kill_completion(&mut child);
            let _ = child.wait();
            #[cfg(unix)]
            COMPLETION_CHILD.store(0, std::sync::atomic::Ordering::SeqCst);
            return Err(io::Error::new(io::ErrorKind::TimedOut, "help timed out"));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let mut bytes = Vec::new();
    fs::File::open(scratch)?
        .take(256 * 1024)
        .read_to_end(&mut bytes)?;
    Ok((String::from_utf8_lossy(&bytes).into_owned(), success))
}

fn safe_command_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_' | b'.' | b'+'))
}

fn system_service_path(path: &Path) -> bool {
    ["/sbin", "/usr/sbin", "/usr/libexec", "/System"]
        .iter()
        .any(|root| path.starts_with(root))
}

fn command_is_system_service(name: &str) -> bool {
    env::split_paths(&env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join(name))
        .find(|path| fs::metadata(path).is_ok_and(|meta| meta.is_file()))
        .and_then(|path| fs::canonicalize(path).ok())
        .is_some_and(|path| system_service_path(&path))
}

fn cache_generated_completion(name: &str, help: &str, directory: &Path, scratch: &Path) -> bool {
    if !safe_command_name(name) {
        return false;
    }
    let mut found = false;
    for shell in ["zsh", "bash", "fish"] {
        let mut shell_found = false;
        let Some(interpreter) = std::iter::once(PathBuf::from(format!("/bin/{shell}")))
            .chain(
                env::split_paths(&env::var_os("PATH").unwrap_or_default()).map(|p| p.join(shell)),
            )
            .find(|p| p.is_file())
        else {
            continue;
        };
        'generators: for generator in ["completion", "completions", "generate-completion"] {
            let advertised = help.lines().any(|line| {
                line.starts_with(char::is_whitespace)
                    && line
                        .trim_start()
                        .strip_prefix(generator)
                        .is_some_and(|rest| {
                            rest.starts_with(char::is_whitespace) || rest.starts_with(':')
                        })
            });
            if !advertised {
                continue;
            }
            let joined = format!("--shell={shell}");
            for arguments in [
                vec![generator, shell],
                vec![generator, "-s", shell],
                vec![generator, "--shell", shell],
                vec![generator, &joined],
            ] {
                let Ok(script) = help_output(name, &arguments, scratch, true) else {
                    continue;
                };
                let recognizable = match shell {
                    "zsh" => {
                        script
                            .lines()
                            .next()
                            .is_some_and(|l| l.starts_with("#compdef "))
                            && script.contains("compdef ")
                    }
                    "bash" => script.contains("complete "),
                    _ => script.contains("complete "),
                };
                if !recognizable || !script.contains(name) {
                    continue;
                }
                if fs::create_dir_all(directory).is_err() {
                    return found;
                }
                let temporary = directory.join(format!(".{name}-{shell}-{}", std::process::id()));
                let valid = fs::write(&temporary, script).is_ok()
                    && help_output_status(
                        &interpreter.to_string_lossy(),
                        &["-n", &temporary.to_string_lossy()],
                        scratch,
                        true,
                    )
                    .is_ok_and(|(_, success)| success);
                if valid
                    && fs::rename(&temporary, directory.join(format!("{name}.{shell}"))).is_ok()
                {
                    found = true;
                    shell_found = true;
                    break 'generators;
                }
                let _ = fs::remove_file(temporary);
            }
        }
        if !shell_found {
            let _ = fs::remove_file(directory.join(format!("{name}.{shell}")));
        }
    }
    found
}

fn cache_completion_protocol(name: &str, help: &str, directory: &Path, scratch: &Path) -> bool {
    if !safe_command_name(name)
        || !help.contains("Available Commands:")
        || !(help.contains("Flags:") || help.contains("Global Flags:"))
    {
        return false;
    }
    let Ok(output) = help_output(name, &["__complete", ""], scratch, true) else {
        return false;
    };
    if !output.lines().any(|line| {
        line.strip_prefix(':')
            .is_some_and(|number| number.parse::<u32>().is_ok())
    }) {
        return false;
    }
    if fs::create_dir_all(directory).is_ok() {
        return fs::write(directory.join(name), "cobra\n").is_ok();
    }
    false
}

fn help_identity<'a>(name: &str, help: &'a str) -> &'a str {
    let mut offset = 0;
    for line in help.split_inclusive('\n') {
        let text = line.trim_end();
        let heading = text.trim_end_matches(':').to_ascii_lowercase();
        if !line.starts_with(char::is_whitespace)
            && (heading == "usage"
                || heading.starts_with("usage:")
                || heading.contains("commands")
                || text == name
                || text.starts_with(&format!("{name} ")))
        {
            return help[offset..].trim();
        }
        offset += line.len();
    }
    help.trim()
}

fn checked_help(name: &str, args: &[&str], scratch: &Path) -> io::Result<String> {
    let (text, success) = help_output_status(name, args, scratch, true)?;
    let invocation = std::iter::once(name)
        .chain(args.iter().take(args.len().saturating_sub(1)).copied())
        .collect::<Vec<_>>()
        .join(" ");
    let identity = help_identity(name, &text);
    if !success
        && help_invocation(&text).is_none()
        && parse_help_options(&text).is_empty()
        && parse_help_subcommands(&invocation, &text).is_empty()
        && identity != invocation
        && !identity.starts_with(&format!("{invocation}\n"))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "help command failed without recognizable help",
        ));
    }
    Ok(text)
}

fn cache_help_tree(
    name: &str,
    help: &str,
    entries: &[(String, &str)],
    config: &Path,
    scratch: &Path,
) -> io::Result<usize> {
    let mut pending = VecDeque::from([(
        vec![name.to_owned()],
        Some(Arc::<str>::from(help_identity(name, help))),
        Vec::<Arc<str>>::new(),
    )]);
    let mut total = 0;
    let mut contexts = 0;
    while let Some((words, help, mut ancestors)) = pending.pop_front() {
        let help = if let Some(help) = help {
            help
        } else {
            let mut args: Vec<_> = words.iter().skip(1).map(String::as_str).collect();
            args.push("--help");
            let first = checked_help(name, &args, scratch);
            let mut child_help = first.as_ref().cloned().unwrap_or_default();
            if parse_help_options(&child_help).is_empty()
                || parse_help_subcommands(&words.join(" "), &child_help).is_empty()
            {
                *args.last_mut().unwrap() = "-h";
                if let Ok(short) = checked_help(name, &args, scratch) {
                    if child_help.is_empty()
                        || (!parse_help_subcommands(&words.join(" "), &short).is_empty()
                            && parse_help_subcommands(&words.join(" "), &child_help).is_empty())
                    {
                        child_help = short;
                    }
                }
            }
            if child_help.is_empty() && first.is_err() {
                eprintln!(
                    "rfig: {}: help unavailable; keeping cached data",
                    words.join(" ")
                );
                continue;
            }
            Arc::from(help_identity(name, &child_help))
        };
        let repeated = ancestors
            .iter()
            .any(|parent| parent.as_ref() == help.as_ref());
        let invocation = words.join(" ");
        let children = if repeated {
            Vec::new()
        } else {
            parse_help_subcommands(&invocation, &help)
        };
        let mut options: Vec<_> = if repeated {
            Vec::new()
        } else if words.len() == 1 {
            entries.to_vec()
        } else {
            parse_help_options(&help).into_iter().collect()
        };
        options.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let mut records = children
            .iter()
            .map(|child| format!("{child}\tsubcommand\n"))
            .collect::<String>();
        for (option, kind) in &options {
            records.push_str(&format!("{option}\t{kind}\n"));
        }
        let file = engine::help_file(config, &words)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid help path"))?;
        fs::create_dir_all(file.parent().unwrap())?;
        let temporary = file.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&temporary, records)?;
        fs::rename(temporary, file)?;
        total += children.len() + options.len();
        contexts += 1;
        if repeated {
            eprintln!("rfig: {invocation}: parent help repeated; stopping this branch");
            continue;
        }
        ancestors.push(help.clone());
        for child in children {
            let mut path = words.clone();
            path.push(child.to_owned());
            pending.push_back((path, None, ancestors.clone()));
        }
        if contexts % 50 == 0 {
            eprintln!(
                "rfig: {name}: {contexts} contexts analyzed, {} pending",
                pending.len()
            );
        }
    }
    Ok(total)
}

fn analyze_command(name: &str, directory: &Path) -> io::Result<usize> {
    if !safe_command_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid command name",
        ));
    }
    if command_is_system_service(name) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing to execute a macOS system service for completion",
        ));
    }
    fs::create_dir_all(directory)?;
    let scratch = directory.join(format!(".help-{}", std::process::id()));
    let first_help = checked_help(name, &["--help"], &scratch);
    let mut help = first_help.as_ref().cloned().unwrap_or_default();
    let mut got_help = first_help.is_ok();
    let mut separate_options = help_advertises_option_command(name, &help);
    let mut kinds = parse_help_options(&help);
    if kinds.is_empty() || parse_help_subcommands(name, &help).is_empty() {
        if let Ok(short_help) = checked_help(name, &["-h"], &scratch) {
            got_help = true;
            separate_options |= help_advertises_option_command(name, &short_help);
            if kinds.is_empty() {
                kinds = parse_help_options(&short_help);
            }
            if parse_help_subcommands(name, &help).is_empty()
                && !parse_help_subcommands(name, &short_help).is_empty()
            {
                help = short_help;
            }
        }
    }
    if let Some(args) = advertised_full_help(&help) {
        let result = help_output_status(name, &args, &scratch, true);
        let full_help = match result {
            Ok((output, true)) if !parse_help_subcommands(name, &output).is_empty() => output,
            result => {
                let _ = fs::remove_file(&scratch);
                return Err(result.err().unwrap_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "advertised full help unavailable; preserving cached completions",
                    )
                }));
            }
        };
        kinds.extend(parse_help_options(&full_help));
        separate_options |= help_advertises_option_command(name, &full_help);
        help.push('\n');
        help.push_str(&full_help);
    }
    if separate_options {
        if let Ok(help) = help_output(name, &["options"], &scratch, true) {
            kinds.extend(parse_help_options(&help));
        }
    }
    if !got_help || (kinds.is_empty() && parse_help_subcommands(name, &help).is_empty()) {
        let _ = fs::remove_file(&scratch);
        if !got_help {
            return Err(first_help
                .err()
                .unwrap_or_else(|| io::Error::other("help unavailable")));
        }
        let config = directory.parent().unwrap();
        for stale in [
            directory.join(format!("{name}.tsv")),
            config.join("fallback").join(format!("{name}.tsv")),
            config.join("generated").join(format!("{name}.zsh")),
            config.join("generated").join(format!("{name}.bash")),
            config.join("generated").join(format!("{name}.fish")),
            config.join("protocol").join(name),
        ] {
            let _ = fs::remove_file(stale);
        }
        fs::create_dir_all(config.join("fallback"))?;
        fs::write(config.join("fallback").join(format!("{name}.tsv")), "")?;
        let file = engine::help_file(config, &[name.to_owned()]).unwrap();
        fs::create_dir_all(file.parent().unwrap())?;
        fs::write(file, "")?;
        return Ok(0);
    }
    let config = directory.parent().unwrap();
    if !cache_generated_completion(name, &help, &config.join("generated"), &scratch) && got_help {
        for shell in ["zsh", "bash", "fish"] {
            let _ = fs::remove_file(config.join("generated").join(format!("{name}.{shell}")));
        }
    }
    if !cache_completion_protocol(name, &help, &config.join("protocol"), &scratch) && got_help {
        let _ = fs::remove_file(config.join("protocol").join(name));
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
    let fallback = directory.parent().unwrap().join("fallback");
    fs::create_dir_all(&fallback)?;
    let subcommands = parse_help_subcommands(name, &help);
    let mut result = subcommands
        .iter()
        .map(|command| format!("{command}\tsubcommand\n"))
        .collect::<String>();
    for (option, kind) in &entries {
        result.push_str(&format!("{option}\t{kind}\n"));
    }
    let temporary = fallback.join(format!(".{name}-{}", std::process::id()));
    fs::write(&temporary, result)?;
    fs::rename(temporary, fallback.join(format!("{name}.tsv")))?;
    let result = cache_help_tree(name, &help, &entries, config, &scratch);
    let _ = fs::remove_file(&scratch);
    result
}

fn analyze_catalog(name: Option<&str>) -> io::Result<()> {
    let name = name.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "specify one command: rfig analyze <command>",
        )
    })?;
    let home = env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    let directory = PathBuf::from(home).join(".config/rfig/options");
    println!(
        "{name}: {} candidates analyzed",
        analyze_command(name, &directory)?
    );
    Ok(())
}

fn enrich_background() -> io::Result<()> {
    let home = PathBuf::from(
        env::var_os("HOME")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?,
    );
    let resolved_home = fs::canonicalize(&home)?;
    let config = home.join(".config/rfig");
    let lock = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(config.join("enrich.lock"))?;
    // One worker per installation, even when setup is run from multiple terminals.
    #[cfg(unix)]
    if unsafe { flock(lock.as_raw_fd(), 2 | 4) } != 0 {
        return Ok(());
    }
    #[cfg(windows)]
    let _lock = platform::lock_file(lock)?;
    if let Err(error) = sandbox::check() {
        fs::write(
            config.join("enrich.status"),
            format!("unavailable\n{error}\n"),
        )?;
        return Err(error);
    }
    let commands = fs::read_to_string(config.join("commands.txt"))?;
    let rules_modified = env::current_exe()
        .ok()
        .and_then(|p| fs::metadata(p).ok())
        .and_then(|m| m.modified().ok());
    let supported: HashSet<String> = fs::read_to_string(config.join("supported.txt"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut done = 0;
    let mut skipped = 0;
    let mut failed = 0;
    for name in commands.lines() {
        if !safe_command_name(name) || supported.contains(name) {
            skipped += 1;
            continue;
        }
        let executable = env::split_paths(&env::var_os("PATH").unwrap_or_default())
            .map(|directory| directory.join(name))
            .find(|path| fs::metadata(path).is_ok_and(|meta| platform::is_executable(path, meta)));
        let Some(path) = executable.and_then(|path| fs::canonicalize(path).ok()) else {
            skipped += 1;
            continue;
        };
        if system_service_path(&path)
            || path.starts_with("/bin")
            || path.starts_with("/usr/bin")
            || path.starts_with("/Library")
            || path.starts_with("/Applications")
            || !([
                "bin",
                ".local",
                ".cargo",
                "go/bin",
                "Library/pnpm",
                ".nix-profile",
            ]
            .iter()
            .any(|root| path.starts_with(resolved_home.join(root)))
                || path.starts_with("/opt/homebrew")
                || path.starts_with("/usr/local")
                || path.starts_with("/opt/podman"))
        {
            skipped += 1;
            continue;
        }
        let cached = config.join("fallback").join(format!("{name}.tsv"));
        if fs::metadata(&cached)
            .ok()
            .zip(fs::metadata(&path).ok())
            .is_some_and(|(cache, binary)| {
                cache.modified().ok() >= binary.modified().ok()
                    && cache.modified().ok() >= rules_modified
            })
        {
            skipped += 1;
            continue;
        }
        if analyze_command(name, &config.join("options")).is_ok() {
            done += 1
        } else {
            failed += 1
        }
        let _ = fs::write(
            config.join("enrich.status"),
            format!("running\n{done} analyzed\n{skipped} skipped\n{failed} failed\n{name}\n"),
        );
    }
    fs::write(
        config.join("enrich.status"),
        format!("done\n{done} analyzed\n{skipped} skipped\n{failed} failed\n"),
    )
}

fn external_completion(name: &str, line: &str) -> io::Result<Option<String>> {
    if !safe_command_name(name) || line.chars().any(char::is_control) {
        return Ok(None);
    }
    let home = PathBuf::from(env::var_os("HOME").unwrap_or_default());
    let xdg_config = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let xdg_data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    let fish_paths = [
        xdg_config.join("fish/completions"),
        PathBuf::from("/opt/homebrew/etc/fish/completions"),
        PathBuf::from("/usr/local/etc/fish/completions"),
        xdg_data.join("fish/vendor_completions.d"),
        PathBuf::from("/opt/homebrew/share/fish/vendor_completions.d"),
        PathBuf::from("/usr/local/share/fish/vendor_completions.d"),
        PathBuf::from("/usr/share/fish/vendor_completions.d"),
    ];
    let config = home.join(".config/rfig");
    fs::create_dir_all(&config)?;
    let scratch = config.join(format!(".external-{}", std::process::id()));
    if fs::read_to_string(config.join("protocol").join(name))
        .ok()
        .as_deref()
        == Some("cobra\n")
    {
        let context = engine::context(line);
        let mut words = context.words;
        words.push(context.prefix);
        let mut args = vec!["__complete"];
        args.extend(words.iter().skip(1).map(String::as_str));
        let result = help_output(name, &args, &scratch, false);
        let _ = fs::remove_file(&scratch);
        if let Ok(output) = result {
            let mut records = Vec::new();
            let mut directive = false;
            for record in output.lines() {
                if record
                    .strip_prefix(':')
                    .is_some_and(|number| number.parse::<u32>().is_ok())
                {
                    directive = true;
                    break;
                }
                records.push(record);
            }
            if directive {
                return Ok(Some(records.join("\n")));
            }
        }
    }
    let fish_file = format!("{name}.fish");
    if fish_paths.iter().any(|dir| dir.join(&fish_file).is_file()) {
        let result = help_output(
            "fish",
            &["-c", "complete -C \"$argv[1]\" 2>/dev/null", "--", line],
            &scratch,
            false,
        );
        let _ = fs::remove_file(&scratch);
        return Ok(result.ok());
    }
    let bash_paths = [
        xdg_data.join("bash-completion/completions"),
        home.join(".bash_completion.d"),
        PathBuf::from("/opt/homebrew/etc/bash_completion.d"),
        PathBuf::from("/usr/local/etc/bash_completion.d"),
        PathBuf::from("/usr/share/bash-completion/completions"),
        PathBuf::from("/etc/bash_completion.d"),
    ];
    let Some(file) = bash_paths
        .iter()
        .flat_map(|dir| [dir.join(name), dir.join(format!("{name}.bash"))])
        .find(|file| file.is_file())
    else {
        return Ok(None);
    };
    const BASH_BRIDGE: &str = r#"
        for base in /opt/homebrew/etc/profile.d/bash_completion.sh /usr/local/etc/profile.d/bash_completion.sh /usr/share/bash-completion/bash_completion; do
          if [[ -r $base ]]; then source "$base" >/dev/null 2>&1; break; fi
        done
        source "$1" >/dev/null 2>&1 || exit 0
        spec=$(complete -p "$2" 2>/dev/null) || exit 0
        [[ $spec =~ -F[[:space:]]+([a-zA-Z_][a-zA-Z_0-9]*) ]] || exit 0
        handler=${BASH_REMATCH[1]}
        COMP_LINE=$3 COMP_POINT=${#3} COMP_TYPE=9
        read -r -a COMP_WORDS <<< "$COMP_LINE"
        [[ $COMP_LINE == *' ' ]] && COMP_WORDS+=( '' )
        COMP_CWORD=$(( ${#COMP_WORDS[@]} - 1 ))
        COMPREPLY=()
        "$handler" "$2" "${COMP_WORDS[COMP_CWORD]}" "${COMP_WORDS[COMP_CWORD-1]}" >/dev/null 2>&1
        printf '%s\n' "${COMPREPLY[@]}"
    "#;
    let result = help_output(
        "/bin/bash",
        &[
            "--noprofile",
            "--norc",
            "-c",
            BASH_BRIDGE,
            "rfig",
            &file.to_string_lossy(),
            name,
            line,
        ],
        &scratch,
        false,
    );
    let _ = fs::remove_file(&scratch);
    Ok(result.ok())
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
            .unwrap_or_else(|| "补全候选".into());
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
    tty: BufWriter<platform::TerminalOutput>,
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
    let mut tty = BufWriter::new(platform::terminal_output()?);
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
            if let Event::Key(key) = input::read()? {
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
        None | Some("-h" | "--help" | "help") => {
            print!("{HELP}");
            return Ok(0);
        }
        Some("completion") => {
            let shell = args.next();
            if matches!(shell.as_deref(), Some("-h" | "--help")) {
                print!("{HELP}");
                return Ok(0);
            }
            match shell.as_deref() {
                Some("zsh") => print!("{ZSH_COMPLETION}"),
                Some("bash") => print!("{}", include_str!("../shell/rfig-completion.bash")),
                Some("fish") => print!("{}", include_str!("../shell/rfig-completion.fish")),
                Some("powershell" | "pwsh") => {
                    print!("{}", include_str!("../shell/rfig-completion.ps1"))
                }
                _ => {
                    eprintln!("usage: rfig completion zsh|bash|fish|powershell");
                    return Ok(2);
                }
            }
            return Ok(0);
        }
        Some("terminal-keys") => {
            if matches!(args.next().as_deref(), Some("-h" | "--help")) {
                print!("{HELP}");
                return Ok(0);
            }
            terminal_app_keys()?;
            return Ok(0);
        }
        Some("setup") => {
            let options: Vec<_> = args.collect();
            if options.iter().any(|s| s == "-h" || s == "--help") {
                print!("{HELP}");
                return Ok(0);
            }
            let shell = match options.as_slice() {
                [] => None,
                [flag, shell] if flag == "--shell" => Some(shell.as_str()),
                _ => {
                    eprintln!("usage: rfig setup [--shell zsh|bash|fish|powershell]");
                    return Ok(2);
                }
            };
            setup(shell)?;
            return Ok(0);
        }
        Some("init") => {
            if matches!(args.next().as_deref(), Some("-h" | "--help")) {
                print!("{HELP}");
                return Ok(0);
            }
            initialize_catalog()?;
            return Ok(0);
        }
        Some("analyze") => {
            let name = args.next();
            if matches!(name.as_deref(), Some("-h" | "--help")) {
                print!("{HELP}");
                return Ok(0);
            }
            analyze_catalog(name.as_deref())?;
            return Ok(0);
        }
        Some("enrich-background") => {
            enrich_background()?;
            return Ok(0);
        }
        Some("external") => {
            let (Some(name), Some(line)) = (args.next(), args.next()) else {
                return Ok(2);
            };
            let ctx = engine::context(&line);
            let cached = if ctx.words.first() == Some(&name) {
                engine::cached_items(&engine::config(), &ctx.words)
            } else {
                Vec::new()
            };
            if let Some(output) = external_completion(&name, &line)?
                .filter(|rows| !engine::prefer_cached(&cached, rows))
            {
                println!("provider");
                let prefix = line.split_whitespace().last().unwrap_or("");
                let prefix = if line.ends_with(' ') { "" } else { prefix };
                for record in output.lines() {
                    let (label, description) = record.split_once('\t').unwrap_or((record, ""));
                    if label.starts_with(prefix)
                        && !label.is_empty()
                        && !label.chars().any(char::is_control)
                        && !description.chars().any(char::is_control)
                    {
                        println!("{label}\t{description}");
                    }
                }
            } else {
                println!("cache");
                if ctx.words.first() == Some(&name) {
                    for item in cached {
                        if item.label.starts_with(&ctx.prefix) {
                            println!("{}\t{}", item.label, item.kind);
                        }
                    }
                }
            }
            return Ok(0);
        }
        Some("remember") => {
            let Some(line) = args.next() else {
                return Ok(2);
            };
            let directory = env::var("PWD").unwrap_or_else(|_| {
                env::current_dir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
            engine::Memory::load(engine::config(), directory).executed(&line);
            return Ok(0);
        }
        Some("query") => {
            let (Some(shell), Some(snapshot), Some(line)) = (args.next(), args.next(), args.next())
            else {
                return Ok(2);
            };
            let snapshot = PathBuf::from(snapshot);
            let (context, items) =
                engine::complete(&shell, &snapshot, &line, &engine::names(&snapshot));
            println!("{}\t{}", context.start, context.injected);
            for item in items {
                println!("{}\t{}\t{}", item.label, item.description, item.kind);
            }
            return Ok(0);
        }
        Some("edit") => {
            let (Some(shell), Some(snapshot), Some(line), Some(point), Some(displayed)) = (
                args.next(),
                args.next(),
                args.next(),
                args.next(),
                args.next(),
            ) else {
                return Ok(2);
            };
            let Some(mut point) = point.parse::<usize>().ok() else {
                return Ok(2);
            };
            let Some(mut displayed) = displayed.parse::<usize>().ok() else {
                return Ok(2);
            };
            if shell == "fish" {
                point = byte_cursor(&line, point).unwrap_or(line.len());
                displayed = byte_cursor(&line, displayed).unwrap_or(line.len());
            }
            editor::run(shell, PathBuf::from(snapshot), line, point, displayed)?;
            return Ok(0);
        }
        Some("position") => {
            let (x, _) = cursor::position()?;
            write!(io::stderr(), "{x}")?;
            return Ok(0);
        }
        Some("pick") => {}
        _ => {
            eprintln!("{HELP}");
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
    platform::initialize_environment();
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
    fn system_services_are_not_help_probed() {
        assert!(system_service_path(Path::new("/usr/sbin/bluetoothd")));
        assert!(system_service_path(Path::new("/System/Library/example")));
        assert!(!system_service_path(Path::new("/opt/homebrew/bin/gh")));
    }

    #[test]
    fn terminal_commands_send_unique_sequences() {
        assert_eq!(TERMINAL_KEY_MAP.len(), 9);
        let sequences: HashSet<_> = TERMINAL_KEY_MAP
            .iter()
            .map(|(_, _, sequence)| *sequence)
            .collect();
        assert_eq!(sequences.len(), TERMINAL_KEY_MAP.len());
        assert!(TERMINAL_KEY_MAP.iter().all(|(_, placeholder, _)| {
            placeholder.starts_with("__RFIG_") && !placeholder.is_empty()
        }));
    }

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
    fn help_lists_subcommands_in_usage_and_command_sections() {
        let usage = "Usage:\n  tool setup   Interactive setup\n  tool start [--port <port>]\n  tool --help\n";
        assert_eq!(
            parse_help_subcommands("tool", usage),
            vec!["setup", "start"]
        );

        let section = "Available Commands:\n  build       Build things\n  deploy      Deploy things\n\nFlags:\n  -h, --help  Help\n";
        assert_eq!(
            parse_help_subcommands("other", section),
            vec!["build", "deploy"]
        );
    }

    #[test]
    fn help_lists_subcommands_in_unpunctuated_sections() {
        let help = "USAGE\n  tool [flags] <subcommand>\n\nSUBCOMMANDS\n  up       Connect\n  status   Show status\n\nFLAGS\n  --socket value  Socket path\n";
        assert_eq!(parse_help_subcommands("tool", help), vec!["up", "status"]);
    }

    #[test]
    fn nested_help_strips_the_full_invocation_and_ignores_wrapped_prose() {
        let help = "Usage: short workers versions <command>\n\nCommands\n  short workers versions list  List versions\n                               with pagination\n  short workers versions get <id>  Get version\n\nOptions:\n  --help  Help\n";
        assert_eq!(
            parse_help_subcommands("wrapper workers versions", help),
            vec!["list", "get"]
        );
    }

    #[test]
    fn nested_help_rejects_another_context_and_reads_wrapped_types() {
        assert!(
            parse_help_subcommands("tool missing", "Commands\n  tool workers  Workers\n")
                .is_empty()
        );
        let kinds = parse_help_options("  -z, --zone  Zone ID\n                         [string]\n  --quiet  Quiet [boolean]\n");
        assert_eq!(kinds.get("--zone"), Some(&"option"));
        assert_eq!(kinds.get("-z"), Some(&"option"));
        assert_eq!(kinds.get("--quiet"), Some(&"flag"));
    }

    #[test]
    fn help_advertises_separate_global_options() {
        assert!(help_advertises_option_command(
            "tool",
            "Use \"tool options\" for a list of global command-line options."
        ));
        assert!(!help_advertises_option_command(
            "other",
            "Use \"tool options\" for a list of global command-line options."
        ));
    }

    #[test]
    fn help_uses_declared_invocation_alias_and_full_reference() {
        let help = "Wrapper CLI\nUsage: short <command> [options]\n\nStart here:\n  short setup  Configure\n  short start  Start\n\nExplore:\n  provider account models\n\nMore help:\n  short help --all  Complete command reference\n";
        assert_eq!(
            parse_help_subcommands("wrapper", help),
            vec!["setup", "start", "help"]
        );
        assert_eq!(advertised_full_help(help), Some(vec!["help", "--all"]));
        assert_eq!(
            advertised_full_help("Examples:\n  stranger help --all\n"),
            None
        );
        assert_eq!(
            parse_help_subcommands(
                "wrapper",
                "Usage: short <command>\n  stranger delete  Unrelated\n"
            ),
            Vec::<&str>::new()
        );
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
