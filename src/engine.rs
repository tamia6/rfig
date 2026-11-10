//! Shared completion, history and ranking for shell adapters.
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub description: String,
    pub kind: String,
    pub popular: usize,
    pub insertion: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Context {
    pub words: Vec<String>,
    pub start: usize,
    pub prefix: String,
    pub injected: bool,
    pub shell: String,
}

// Tokenization is deliberately non-executing: never evaluate user input to find words.
pub fn context(line: &str) -> Context {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut start = 0;
    let mut active = false;
    let mut quote = None;
    let mut escape = false;
    for (i, c) in line.char_indices() {
        if escape {
            word.push(c);
            escape = false;
            continue;
        }
        if !active && !c.is_whitespace() {
            start = i;
            active = true;
        }
        if c == '\\' && quote != Some('\'') {
            escape = true;
            continue;
        }
        if quote == Some(c) {
            quote = None;
            continue;
        }
        if quote.is_none() && (c == '\'' || c == '"') {
            quote = Some(c);
            continue;
        }
        if quote.is_none() && "|;&".contains(c) {
            words.clear();
            word.clear();
            active = false;
            start = i + c.len_utf8();
        } else if quote.is_none() && c.is_whitespace() {
            if active {
                words.push(std::mem::take(&mut word));
            }
            active = false;
            start = i + c.len_utf8();
        } else {
            word.push(c);
        }
    }
    Context {
        words,
        start,
        prefix: word,
        injected: false,
        shell: String::new(),
    }
}

pub fn quote(text: &str, typed: &str) -> String {
    let mut out = String::new();
    for (index, c) in text.chars().enumerate() {
        // Retain a home expansion the user typed, without expanding literal candidates.
        let home = index == 0 && c == '~' && text.starts_with("~/") && typed.starts_with("~/");
        if !(home || c.is_alphanumeric() || "_./-:=+@%".contains(c)) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
pub fn quote_for_shell(text: &str, typed: &str, shell: &str) -> String {
    if shell == "powershell" {
        if text
            .chars()
            .all(|c| c.is_alphanumeric() || "_./\\-:=+@%".contains(c))
        {
            return text.into();
        }
        return format!("'{}'", text.replace('\'', "''"));
    }
    quote(text, typed)
}
fn valid(text: &str) -> bool {
    !text.chars().any(char::is_control)
}
pub fn config() -> PathBuf {
    crate::platform::home().join(".config/rfig")
}

pub fn help_file(root: &Path, words: &[String]) -> Option<PathBuf> {
    if words.is_empty() || words.iter().any(|word| !super::safe_command_name(word)) {
        return None;
    }
    let mut file = root.join("help").join(&words[0]);
    if words.len() == 1 {
        file.push("root.tsv");
    } else {
        file.push("subcommands");
        for word in &words[1..words.len() - 1] {
            file.push(word);
        }
        file.push(format!("{}.tsv", words.last()?));
    }
    Some(file)
}

pub fn cached_items(root: &Path, words: &[String]) -> Vec<Item> {
    fn read(file: &Path) -> Vec<Item> {
        fs::read_to_string(file)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let (label, kind) = line.split_once('\t')?;
                if label.is_empty()
                    || !valid(label)
                    || !matches!(kind, "subcommand" | "option" | "flag")
                {
                    return None;
                }
                Some(Item {
                    label: label.into(),
                    description: String::new(),
                    kind: kind.into(),
                    popular: 0,
                    insertion: None,
                })
            })
            .collect()
    }
    let Some(file) = help_file(root, &words[..words.len().min(1)]) else {
        return Vec::new();
    };
    let mut items = if file.is_file() {
        read(&file)
    } else if words.len() == 1 {
        read(&root.join("fallback").join(format!("{}.tsv", words[0])))
    } else {
        return Vec::new();
    };
    let mut path = vec![words[0].clone()];
    let mut value = false;
    for word in &words[1..] {
        if value {
            value = false;
            continue;
        }
        if word == "--" {
            return Vec::new();
        }
        if word.starts_with('-') {
            let (option, inline) = word
                .split_once('=')
                .map_or((word.as_str(), false), |(key, _)| (key, true));
            let Some(item) = items
                .iter()
                .find(|item| item.label == option && item.kind != "subcommand")
            else {
                return Vec::new();
            };
            value = item.kind == "option" && !inline;
            continue;
        }
        if !items
            .iter()
            .any(|item| item.label == *word && item.kind == "subcommand")
        {
            return Vec::new();
        }
        path.push(word.clone());
        let Some(file) = help_file(root, &path) else {
            return Vec::new();
        };
        items = read(&file);
    }
    if value {
        Vec::new()
    } else {
        items
    }
}

pub fn prefer_cached(cached: &[Item], source: &str) -> bool {
    if cached.is_empty() {
        return false;
    }
    let mut labels = source
        .lines()
        .map(|row| row.split('\t').next().unwrap_or(""))
        .filter(|label| !label.is_empty())
        .peekable();
    labels.peek().is_none()
        || (cached.iter().any(|item| item.kind == "subcommand")
            && labels.all(|label| Path::new(label).exists()))
}

pub struct Memory {
    pub history: Vec<String>,
    pub scores: HashMap<(String, String), u64>,
    directory: String,
    root: PathBuf,
}
impl Memory {
    pub fn load(root: PathBuf, directory: String) -> Self {
        let history = fs::read_to_string(root.join("history.tsv"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once('\x1f'))
            .filter(|(dir, l)| *dir == directory && valid(l))
            .map(|(_, l)| l.to_owned())
            .collect();
        let mut scores = HashMap::new();
        for line in fs::read_to_string(root.join("usage.log"))
            .unwrap_or_default()
            .lines()
        {
            let parts: Vec<_> = line.split('\x1f').collect();
            if parts.len() == 3 && matches!(parts[0], "C" | "R") {
                *scores
                    .entry((parts[1].to_owned(), parts[2].to_owned()))
                    .or_insert(0) += if parts[0] == "C" { 3 } else { 1 };
            }
        }
        Self {
            history,
            scores,
            directory,
            root,
        }
    }
    fn append(&self, file: &str, record: &str) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let mut f = crate::platform::private_append(&self.root.join(file))?;
        writeln!(f, "{record}")
    }
    pub fn suggest(&self, line: &str) -> String {
        if line.is_empty() || !valid(line) {
            return String::new();
        }
        self.history
            .iter()
            .rev()
            .find(|l| l.starts_with(line) && *l != line)
            .map(|l| l[line.len()..].to_owned())
            .unwrap_or_default()
    }
    pub fn add_history(&self, line: &str, items: &mut Vec<Item>) {
        let suffix = self.suggest(line);
        if !suffix.is_empty() {
            let label = format!("{line}{suffix}");
            items.insert(
                0,
                Item {
                    label,
                    description: "History".into(),
                    kind: "history".into(),
                    popular: 0,
                    insertion: None,
                },
            );
        }
    }
    pub fn record(&mut self, kind: &str, context: &str, label: &str) {
        if label.is_empty() || !valid(context) || !valid(label) {
            return;
        }
        if self
            .append("usage.log", &format!("{kind}\x1f{context}\x1f{label}"))
            .is_ok()
        {
            *self
                .scores
                .entry((context.to_owned(), label.to_owned()))
                .or_default() += if kind == "C" { 3 } else { 1 };
        }
    }
    pub fn executed(&mut self, line: &str) {
        // Leading-space exclusion consistently protects all new adapters.
        if line.is_empty() || line.starts_with(' ') || !valid(line) || !valid(&self.directory) {
            return;
        }
        if self
            .append("history.tsv", &format!("{}\x1f{line}", self.directory))
            .is_ok()
        {
            self.history.push(line.to_owned());
        }
        let ctx = context(&format!("{line} "));
        let mut prefix = String::new();
        for word in ctx.words {
            self.record("R", &prefix, &word);
            if !prefix.is_empty() {
                prefix.push(' ');
            }
            prefix.push_str(&word);
        }
    }
    pub fn rank(&self, ctx: &Context, items: &mut Vec<Item>) {
        let prefix = ctx.words.join(" ");
        let score = |item: &Item| {
            *self
                .scores
                .get(&(prefix.clone(), item.label.clone()))
                .unwrap_or(&0)
        };
        let mut seen = HashSet::new();
        items.retain(|i| {
            valid(&i.label)
                && !i.label.is_empty()
                && valid(&i.description)
                && (i.insertion.is_some() || i.label.starts_with(&ctx.prefix))
                && seen.insert(i.label.clone())
                && if i.insertion.is_some() {
                    true
                } else if ctx.prefix.starts_with("--") {
                    i.label.starts_with("--")
                } else if ctx.prefix.starts_with('-') {
                    i.label.starts_with('-') && !i.label.starts_with("--")
                } else {
                    true
                }
        });
        items.sort_by(|a, b| {
            score(b)
                .cmp(&score(a))
                .then(a.label.starts_with('-').cmp(&b.label.starts_with('-')))
        });
        for (index, item) in items.iter_mut().enumerate() {
            item.popular = if index < 3 && score(item) > 0 {
                index + 1
            } else {
                0
            };
        }
    }
}

pub fn names(snapshot: &Path) -> Vec<String> {
    let mut result: Vec<String> = fs::read_to_string(snapshot.join("names"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    result.extend(
        fs::read_to_string(config().join("commands.txt"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned),
    );
    for dir in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.flatten() {
                if e.metadata()
                    .is_ok_and(|m| crate::platform::is_executable(&e.path(), m))
                {
                    result.push(crate::platform::command_name(
                        &e.file_name().to_string_lossy(),
                    ));
                }
            }
        }
    }
    result.sort();
    result.dedup();
    result
}

fn host_completion(shell: &str, snapshot: &Path, line: &str) -> (Context, Vec<Item>) {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let mut context = context(line);
    context.shell = shell.into();
    let request = snapshot.join("request.tmp");
    if fs::write(&request, format!("{id}\n{line}"))
        .and_then(|_| fs::rename(&request, snapshot.join("request")))
        .is_err()
    {
        return (context, Vec::new());
    }
    let response = snapshot.join(format!("response.{id}"));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !response.is_file() {
        if super::COMPLETION_CANCELLED.load(Ordering::SeqCst) || Instant::now() >= deadline {
            return (context, Vec::new());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let text = fs::read_to_string(&response).unwrap_or_default();
    let _ = fs::remove_file(response);
    let mut rows = text.lines();
    if let Some((start, injected)) = rows.next().and_then(|line| line.split_once('\t')) {
        if let Ok(start) = start.parse::<usize>() {
            if line.is_char_boundary(start) {
                context.start = start;
            }
        }
        context.injected = injected == "true" || injected == "1";
        if context.injected && context.words.is_empty() {
            context.words.push(std::mem::take(&mut context.prefix));
        }
    }
    let items = rows
        .filter_map(|row| {
            let mut fields = row.splitn(4, '\t');
            let label = fields.next()?;
            let insertion = fields.next()?;
            let description = fields.next()?;
            let kind = fields.next()?;
            if label.is_empty() || !valid(label) || !valid(insertion) {
                return None;
            }
            Some(Item {
                label: label.into(),
                insertion: Some(insertion.into()),
                description: description.into(),
                kind: kind.into(),
                popular: 0,
            })
        })
        .collect();
    (context, items)
}

pub fn complete(
    shell: &str,
    snapshot: &Path,
    line: &str,
    names: &[String],
) -> (Context, Vec<Item>) {
    if matches!(shell, "zsh" | "powershell") {
        return host_completion(shell, snapshot, line);
    }
    let mut ctx = context(line);
    ctx.shell = shell.into();
    let mut input = line.to_owned();
    if ctx.words.is_empty() && names.contains(&ctx.prefix) {
        ctx.words.push(std::mem::take(&mut ctx.prefix));
        ctx.start = line.len();
        ctx.injected = true;
        input.push(' ');
    }
    let mut items = Vec::new();
    if ctx.words.is_empty() {
        items.extend(
            names
                .iter()
                .filter(|name| name.starts_with(&ctx.prefix))
                .map(|name| Item {
                    label: name.clone(),
                    description: String::new(),
                    kind: "command".into(),
                    popular: 0,
                    insertion: None,
                }),
        );
        return (ctx, items);
    }
    let name = &ctx.words[0];
    let scratch = snapshot.join("output");
    let provider = if shell == "fish" {
        include_str!("../shell/fish-provider.fish")
    } else {
        include_str!("../shell/bash-provider.bash")
    };
    let snapshot_file = snapshot.join("definitions");
    let mut words = ctx.words.clone();
    words.push(ctx.prefix.clone());
    let executable = env::var("RFIG_SHELL_EXE").unwrap_or_else(|_| shell.to_owned());
    let mut args = if shell == "fish" {
        vec!["--no-config", "-c", provider, "--"]
    } else {
        vec!["--noprofile", "--norc", "-c", provider, "rfig"]
    };
    let path = snapshot_file.to_string_lossy();
    args.extend([path.as_ref(), input.as_str()]);
    args.extend(words.iter().map(String::as_str));
    let native = super::help_output(&executable, &args, &scratch, false).unwrap_or_default();
    let _ = fs::remove_file(&scratch);
    let mut source = native
        .split_once("provider\n")
        .map(|(_, rows)| rows.to_owned());
    if source.is_none() {
        source = super::external_completion(name, &input).ok().flatten();
    }
    let cached = cached_items(&config(), &ctx.words);
    // An empty provider or a group's filesystem fallback must not hide known children.
    source = source.filter(|rows| !prefer_cached(&cached, rows));
    let mut option_kinds: HashMap<String, String> =
        fs::read_to_string(config().join("options").join(format!("{name}.tsv")))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect();
    if ctx.words.len() > 1 {
        option_kinds.clear();
    }
    option_kinds.extend(
        cached
            .iter()
            .filter(|item| item.kind != "subcommand")
            .map(|item| (item.label.clone(), item.kind.clone())),
    );
    if let Some(source) = source {
        for line in source.lines() {
            let (label, desc) = line.split_once('\t').unwrap_or((line, ""));
            // Native providers return literal labels, including spaces in file names.
            let label = label.to_owned();
            let kind = option_kinds.get(&label).cloned().unwrap_or_else(|| {
                if label.starts_with("--") {
                    "option"
                } else if label.starts_with('-') {
                    "flag"
                } else if ctx.words.len() == 1 && !label.contains('/') {
                    "subcommand"
                } else {
                    "argument"
                }
                .to_owned()
            });
            items.push(Item {
                label,
                description: desc.to_owned(),
                kind,
                popular: 0,
                insertion: None,
            });
        }
    } else {
        items = cached;
    }
    (ctx, items)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_provider_preserves_replacement_ranges_and_quoted_insertions() {
        use std::{
            sync::atomic::{AtomicU64, Ordering},
            time::Duration,
        };
        static ID: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "rfig-host-test-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let provider_dir = dir.clone();
        let provider = std::thread::spawn(move || {
            let request = provider_dir.join("request");
            for _ in 0..500 {
                if request.exists() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let request = fs::read_to_string(request).unwrap();
            let (id, text) = request.split_once('\n').unwrap();
            assert_eq!(text, "tool --path=./pa");
            fs::write(
                provider_dir.join(format!("response.{id}")),
                "5\tfalse\nparent dir\t--path=./parent\\ dir/\tDirectory\targument\n",
            )
            .unwrap();
        });
        let (ctx, mut items) = host_completion("zsh", &dir, "tool --path=./pa");
        provider.join().unwrap();
        Memory::load(dir.clone(), "test".into()).rank(&ctx, &mut items);
        assert_eq!(ctx.start, 5);
        assert_eq!(
            items[0].insertion.as_deref(),
            Some("--path=./parent\\ dir/")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn powershell_literals_use_powershell_quoting() {
        assert_eq!(
            quote_for_shell("two words'file", "", "powershell"),
            "'two words''file'"
        );
        assert_eq!(
            quote_for_shell("C:\\work\\file", "", "powershell"),
            "C:\\work\\file"
        );
    }

    #[test]
    fn known_groups_replace_file_fallback_but_leaf_dynamic_values_survive() {
        let group = vec![Item {
            label: "child".into(),
            kind: "subcommand".into(),
            description: String::new(),
            popular: 0,
            insertion: None,
        }];
        assert!(prefer_cached(&group, ".\tDirectory\n"));
        assert!(!prefer_cached(&group, "dynamic-value\n"));
        let leaf = vec![Item {
            label: "--help".into(),
            kind: "flag".into(),
            description: String::new(),
            popular: 0,
            insertion: None,
        }];
        assert!(!prefer_cached(&leaf, ".\tDirectory\n"));
        assert!(prefer_cached(&leaf, "\n"));
    }
    #[test]
    fn tokenization_never_executes() {
        let c = context("echo done && tool user 'alice smith' --ro");
        assert_eq!(c.words, ["tool", "user", "alice smith"]);
        assert_eq!(c.prefix, "--ro");
        assert_eq!(context("tool a\\ b").prefix, "a b");
        assert_eq!(quote("a b;$(no)", ""), "a\\ b\\;\\$\\(no\\)");
    }
    #[test]
    fn completion_preserves_only_typed_home_expansion() {
        assert_eq!(quote("~/space dir/", "~/spa"), "~/space\\ dir/");
        assert_eq!(quote("~/space dir/", ""), "\\~/space\\ dir/");
        assert_eq!(quote("~/space dir/", "\"~/spa"), "\\~/space\\ dir/");
        assert_eq!(quote("~/space dir/", "\\~/spa"), "\\~/space\\ dir/");
        assert_eq!(quote("~/$(touch PWN)/", "~/"), "~/\\$\\(touch\\ PWN\\)/");
    }
    #[test]
    fn history_is_first_without_changing_regular_ranking() {
        let mut memory = Memory::load(PathBuf::from("/nonexistent"), "dir".into());
        memory.history = vec!["tool beta".into(), "tool alpha".into()];
        let mut items = vec![Item {
            label: "beta".into(),
            description: String::new(),
            kind: "subcommand".into(),
            popular: 1,
            insertion: None,
        }];
        memory.add_history("tool", &mut items);
        assert_eq!(items[0].label, "tool alpha");
        assert_eq!(items[0].kind, "history");
        assert_eq!(items[0].popular, 0);
        assert_eq!(items[1].popular, 1);
        let mut absent = Vec::new();
        memory.add_history("", &mut absent);
        memory.add_history("other", &mut absent);
        assert!(absent.is_empty());
    }
    #[test]
    fn dash_filter_and_popular_limit() {
        let mut memory = Memory::load(PathBuf::from("/nonexistent"), "dir".into());
        let mut items: Vec<_> = ["--all", "-a", "run", "info", "show", "list"]
            .iter()
            .map(|s| Item {
                label: s.to_string(),
                description: String::new(),
                kind: "argument".into(),
                popular: 0,
                insertion: None,
            })
            .collect();
        for name in ["run", "info", "show", "list"] {
            memory.scores.insert(("tool".into(), name.into()), 1);
        }
        let mut short = items.clone();
        memory.rank(&context("tool -"), &mut short);
        assert_eq!(short.len(), 1);
        assert_eq!(short[0].label, "-a");
        let mut long = items.clone();
        memory.rank(&context("tool --"), &mut long);
        assert_eq!(long.len(), 1);
        assert_eq!(long[0].label, "--all");
        memory.rank(&context("tool "), &mut items);
        assert_eq!(items.iter().filter(|i| i.popular > 0).count(), 3);
    }
}
