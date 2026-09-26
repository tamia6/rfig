use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::{self, BufWriter, Read, Write},
    os::fd::AsRawFd,
    os::unix::fs::PermissionsExt,
    os::unix::process::CommandExt,
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

unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
    fn setsid() -> i32;
    fn kill(pid: i32, signal: i32) -> i32;
    fn setrlimit(resource: i32, limit: *const ResourceLimit) -> i32;
}

#[repr(C)]
struct ResourceLimit {
    current: u64,
    maximum: u64,
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

Inline command completion for zsh. Suggestions appear as you type.

Commands:
  setup                 Scan commands, install zsh integration, and enrich in the background
  init                  Refresh the local command catalog
  analyze <command>     Refresh completion data for one command
  completion zsh        Print the zsh completion definition for rfig

Options:
  -h, --help            Show this help

Examples:
  rfig setup
  rfig analyze kubectl
  rfig completion zsh
";

const ZSH_COMPLETION: &str = r#"#compdef rfig
_rfig() {
  if (( CURRENT == 2 )); then
    compadd -- setup init analyze completion
  elif (( CURRENT == 3 )) && [[ $words[2] == analyze ]]; then
    local file="$HOME/.config/rfig/commands.txt"
    [[ -r $file ]] || return
    local -a available
    available=(${(f)"$(<"$file")"})
    compadd -- "${available[@]}"
  elif (( CURRENT == 3 )) && [[ $words[2] == completion ]]; then
    compadd -- zsh
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
    let executable = env::current_exe()?;
    let bundled_script = executable
        .parent()
        .and_then(Path::parent)
        .map(|prefix| prefix.join("share/rfig/rfig.zsh"));
    let script = if executable.starts_with(home.join(".local/bin")) && local_script.is_file() {
        local_script.clone()
    } else {
        bundled_script
            .filter(|path| path.is_file())
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
    let mut worker = Command::new(env::current_exe()?);
    worker
        .arg("enrich-background")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
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
    println!("rfig is ready. Completion enrichment is running in the background; open a new zsh terminal.");
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

fn parse_help_subcommands<'a>(name: &str, help: &'a str) -> Vec<&'a str> {
    let mut commands = Vec::new();
    let mut in_section = false;
    for line in help.lines() {
        let trimmed = line.trim();
        if trimmed
            .strip_suffix(':')
            .is_some_and(|heading| heading.to_ascii_lowercase().contains("command"))
        {
            in_section = true;
            continue;
        }
        let candidate = if in_section {
            if line.is_empty() || !line.starts_with(char::is_whitespace) || trimmed.ends_with(':') {
                in_section = false;
                None
            } else {
                trimmed
                    .split_whitespace()
                    .next()
                    .map(|word| word.trim_end_matches(':'))
            }
        } else {
            trimmed
                .strip_prefix(name)
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
    let output = fs::File::create(scratch)?;
    let mut command = if background_safe {
        let mut sandbox = Command::new("/usr/bin/sandbox-exec");
        sandbox.args([
            "-p",
            "(version 1) (allow default) (deny file-write*) (deny network*)",
            name,
        ]);
        sandbox
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
        .stderr(Stdio::from(output));
    unsafe {
        command.pre_exec(|| {
            let file = ResourceLimit {
                current: 512 * 1024,
                maximum: 512 * 1024,
            };
            let cpu = ResourceLimit {
                current: 2,
                maximum: 2,
            };
            if setsid() < 0 || setrlimit(1, &file) < 0 || setrlimit(0, &cpu) < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut child = command.spawn()?;
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        if child.try_wait()?.is_some() {
            unsafe {
                kill(-(child.id() as i32), 9);
            }
            break;
        }
        if Instant::now() >= deadline {
            unsafe {
                kill(-(child.id() as i32), 9);
            }
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

fn safe_command_name(name: &str) -> bool {
    !name.is_empty()
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
    for generator in ["completion", "completions", "generate-completion"] {
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
        for arguments in [
            vec![generator, "zsh"],
            vec![generator, "-s", "zsh"],
            vec![generator, "--shell", "zsh"],
            vec![generator, "--shell=zsh"],
        ] {
            let Ok(script) = help_output(name, &arguments, scratch, true) else {
                continue;
            };
            if !script
                .lines()
                .next()
                .is_some_and(|line| line.starts_with("#compdef "))
                || !script.contains("compdef ")
                || !script.contains(&format!(" {name}"))
            {
                continue;
            }
            if fs::create_dir_all(directory).is_err() {
                return false;
            }
            let temporary = directory.join(format!(".{name}-{}", std::process::id()));
            let valid = fs::write(&temporary, script).is_ok()
                && Command::new("/bin/zsh")
                    .arg("-n")
                    .arg(&temporary)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|status| status.success());
            if valid && fs::rename(&temporary, directory.join(format!("{name}.zsh"))).is_ok() {
                return true;
            }
            let _ = fs::remove_file(temporary);
        }
    }
    false
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
    let first_help = help_output(name, &["--help"], &scratch, true);
    let mut help = first_help.as_ref().cloned().unwrap_or_default();
    let mut got_help = first_help.is_ok();
    let mut separate_options = help_advertises_option_command(name, &help);
    let mut kinds = parse_help_options(&help);
    if kinds.is_empty() || parse_help_subcommands(name, &help).is_empty() {
        if let Ok(short_help) = help_output(name, &["-h"], &scratch, true) {
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
    if separate_options {
        if let Ok(help) = help_output(name, &["options"], &scratch, true) {
            kinds.extend(parse_help_options(&help));
        }
    }
    if !got_help || (kinds.is_empty() && parse_help_subcommands(name, &help).is_empty()) {
        let _ = fs::remove_file(&scratch);
        if !got_help {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "help unavailable"));
        }
        let config = directory.parent().unwrap();
        for stale in [
            directory.join(format!("{name}.tsv")),
            config.join("fallback").join(format!("{name}.tsv")),
            config.join("generated").join(format!("{name}.zsh")),
            config.join("protocol").join(name),
        ] {
            let _ = fs::remove_file(stale);
        }
        fs::create_dir_all(config.join("fallback"))?;
        fs::write(config.join("fallback").join(format!("{name}.tsv")), "")?;
        return Ok(0);
    }
    let config = directory.parent().unwrap();
    if !cache_generated_completion(name, &help, &config.join("generated"), &scratch) && got_help {
        let _ = fs::remove_file(config.join("generated").join(format!("{name}.zsh")));
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
    Ok(entries.len() + subcommands.len())
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
        .open(config.join("enrich.lock"))?;
    // One worker per installation, even when setup is run from multiple terminals.
    if unsafe { flock(lock.as_raw_fd(), 2 | 4) } != 0 {
        return Ok(());
    }
    let commands = fs::read_to_string(config.join("commands.txt"))?;
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
            .find(|path| {
                fs::metadata(path)
                    .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
            });
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
            .is_some_and(|(cache, binary)| cache.modified().ok() >= binary.modified().ok())
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
        let arguments: Vec<_> = line.split_whitespace().skip(1).collect();
        let mut args = vec!["__complete"];
        args.extend(arguments);
        if line.ends_with(' ') || line == name {
            args.push("");
        }
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
            if shell.as_deref() != Some("zsh") {
                eprintln!("usage: rfig completion zsh");
                return Ok(2);
            }
            print!("{ZSH_COMPLETION}");
            return Ok(0);
        }
        Some("setup") => {
            if matches!(args.next().as_deref(), Some("-h" | "--help")) {
                print!("{HELP}");
                return Ok(0);
            }
            setup()?;
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
            if let Some(output) = external_completion(&name, &line)? {
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
            }
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
