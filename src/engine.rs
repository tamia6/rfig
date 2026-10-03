//! Shared completion, history and ranking for the Bash and Fish adapters.
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub description: String,
    pub kind: String,
    pub popular: usize,
}
#[derive(Clone, Debug)]
pub struct Context {
    pub words: Vec<String>,
    pub start: usize,
    pub prefix: String,
    pub injected: bool,
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
    }
}

pub fn quote(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if !(c.is_alphanumeric() || "_./-:=+@%".contains(c)) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
fn valid(text: &str) -> bool {
    !text.chars().any(char::is_control)
}
pub fn config() -> PathBuf {
    PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config/rfig")
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
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(self.root.join(file))?;
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(fs::Permissions::from_mode(0o600))?;
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
                && i.label.starts_with(&ctx.prefix)
                && seen.insert(i.label.clone())
                && if ctx.prefix.starts_with("--") {
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
            use std::os::unix::fs::PermissionsExt;
            for e in entries.flatten() {
                if e.metadata()
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                {
                    result.push(e.file_name().to_string_lossy().into_owned());
                }
            }
        }
    }
    result.sort();
    result.dedup();
    result
}

pub fn complete(
    shell: &str,
    snapshot: &Path,
    line: &str,
    names: &[String],
) -> (Context, Vec<Item>) {
    let mut ctx = context(line);
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
    let option_kinds: HashMap<String, String> =
        fs::read_to_string(config().join("options").join(format!("{name}.tsv")))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect();
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
            });
        }
    } else if ctx.words.len() == 1 && super::safe_command_name(name) {
        for line in fs::read_to_string(config().join("fallback").join(format!("{name}.tsv")))
            .unwrap_or_default()
            .lines()
        {
            if let Some((label, kind)) = line.split_once('\t') {
                items.push(Item {
                    label: label.into(),
                    description: String::new(),
                    kind: kind.into(),
                    popular: 0,
                });
            }
        }
    }
    (ctx, items)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokenization_never_executes() {
        let c = context("echo done && tool user 'alice smith' --ro");
        assert_eq!(c.words, ["tool", "user", "alice smith"]);
        assert_eq!(c.prefix, "--ro");
        assert_eq!(context("tool a\\ b").prefix, "a b");
        assert_eq!(quote("a b;$(no)"), "a\\ b\\;\\$\\(no\\)");
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
