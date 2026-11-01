//! `textsmith bm export`: bookmarks as shell functions, for bash and zsh,
//! so they work in any terminal.
//!
//! In The Textsmith an argument is put into the command as it was typed; a
//! function gets its arguments already split by the shell. So each place
//! becomes a quoted expansion that fits where it stands: "$1" outside quotes,
//! ${1} inside double quotes, '"$1"' inside single quotes. Defaults and places
//! for the rest of the arguments become a few lines of shell code.

use crate::bookmarks::{self as bm, Bookmark, Part};

#[derive(Clone, Copy, PartialEq)]
enum Quote {
    None,
    Single,
    Double,
}

/// The quoting state after a piece of shell text, starting from `q`.
fn after(text: &str, mut q: Quote) -> Quote {
    let mut esc = false;
    for c in text.chars() {
        if esc {
            esc = false;
            continue;
        }
        match q {
            Quote::Single => {
                if c == '\'' {
                    q = Quote::None
                }
            }
            Quote::Double => match c {
                '\\' => esc = true,
                '"' => q = Quote::None,
                _ => {}
            },
            Quote::None => match c {
                '\\' => esc = true,
                '\'' => q = Quote::Single,
                '"' => q = Quote::Double,
                _ => {}
            },
        }
    }
    q
}

/// A variable for a place: _name, or _a2 for the second place without a name.
fn var(b: &Bookmark, i: usize) -> String {
    let s = &b.slots[i];
    if s.name.is_empty() {
        if s.rest { "_rest".into() } else { format!("_a{}", i + 1) }
    } else {
        format!("_{}", s.name.replace(['-', '.'], "_"))
    }
}

/// How a value is used where it stands. `one` is a single positional ("$2");
/// otherwise an array variable.
fn use_at(q: Quote, one: Option<usize>, v: &str) -> String {
    match (one, q) {
        (Some(n), Quote::None) => format!("\"${{{n}}}\""),
        (Some(n), Quote::Double) => format!("${{{n}}}"),
        (Some(n), Quote::Single) => format!("'\"${{{n}}}\"'"),
        (None, Quote::None) => format!("\"${{{v}[@]}}\""),
        (None, Quote::Double) => format!("${{{v}[*]}}"),
        (None, Quote::Single) => format!("'\"${{{v}[*]}}\"'"),
    }
}

/// A default as shell words: a place it names (<to:<from>>) becomes that
/// place's value, through `refer`.
fn default_words(b: &Bookmark, d: &str, refer: impl Fn(usize) -> String) -> String {
    bm::ref_re()
        .replace_all(d, |c: &regex::Captures| match b.slots.iter().position(|s| s.name == c[1]) {
            Some(j) => refer(j),
            None => c[0].to_string(),
        })
        .into_owned()
}

/// A shell-safe single-quoted string.
fn sq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// A bookmark that is only an addition for #@ (`2>/dev/null`, `| sort`) is
/// not a command on its own.
fn is_addition(b: &Bookmark) -> bool {
    let c = b.command.trim_start();
    let c = c.trim_start_matches(|ch: char| ch.is_ascii_digit());
    c.starts_with('>') || c.starts_with('<') || c.starts_with('|') || c.starts_with('&')
}

/// The shell code that finds the machine's name the way The Textsmith does.
fn machine_helper() -> String {
    format!(
        "# The Textsmith's folder and this machine's name, found as The Textsmith finds them.\n__ts_config() {{\n  if [ -n \"${{TEXTSMITH_CONFIG:-$TEXTSMITH_HOME}}\" ]; then printf '%s\\n' \"${{TEXTSMITH_CONFIG:-$TEXTSMITH_HOME}}\"\n  elif [ -d \"$HOME/.textsmith\" ]; then printf '%s\\n' \"$HOME/.textsmith\"\n  else printf '%s\\n' \"${{XDG_CONFIG_HOME:-$HOME/.config}}/textsmith\"; fi\n}}\n__ts_machine() {{\n  if [ -n \"$TEXTSMITH_MACHINE\" ]; then printf '%s\\n' \"$TEXTSMITH_MACHINE\"\n  elif [ -s \"$(__ts_config)/machine\" ]; then head -n 1 \"$(__ts_config)/machine\" | tr -d ' \\t'\n  elif [ -s /etc/hostname ]; then head -n 1 /etc/hostname\n  else hostname; fi\n}}\n\n"
    )
}

/// A bookmark with a command per machine: one function for each command, and
/// one that chooses among them by the machine's name.
fn per_machine(b: &Bookmark, prefix: &str) -> Option<String> {
    let name = format!("{prefix}{}", b.name);
    let mut out = format!("# {}\n", bm::format(b));
    let mut cases = String::new();
    let mut machines: Vec<String> = vec![];
    for (i, v) in b.variants.iter().enumerate() {
        let mut one = bm::parse_on(&b.raw, &v.machines[0]);
        one.command = one.this_command();
        one.variants.clear();
        if is_addition(&one) {
            return None;
        }
        let inner = format!("__ts_{}_{}", b.name.replace(['-', '.'], "_"), i + 1);
        let f = function(&one, "")?;
        // The inner function under its own name, without its comment line.
        let body = f.splitn(2, '\n').nth(1).unwrap_or("").replacen(&format!("{}()", one.name), &format!("{inner}()"), 1);
        out.push_str(&body);
        let pats: Vec<String> = v.machines.iter().map(|m| if m == "*" { "*".to_string() } else { m.clone() }).collect();
        machines.extend(v.machines.iter().map(|m| if m == "*" { "any other machine".to_string() } else { m.clone() }));
        cases.push_str(&format!("    {}) {inner} \"$@\" ;;\n", pats.join("|")));
    }
    if !b.variants.iter().any(|v| v.machines.iter().any(|m| m == "*")) {
        cases.push_str(&format!(
            "    *) printf '%s\\n' {}\"$(__ts_machine)\"{} >&2; return 2 ;;\n",
            sq(&format!("{} has no command for this machine (", b.name)),
            sq(&format!("); it has commands for {}.", machines.join(", ")))
        ));
    }
    out.push_str(&format!("{name}() {{\n  case \"$(__ts_machine)\" in\n{cases}  esac\n}}\n"));
    Some(out)
}

/// One bookmark as a function, or None (no name, or not a command by itself).
pub fn function(b: &Bookmark, prefix: &str) -> Option<String> {
    if b.name.is_empty() {
        return None;
    }
    if !b.variants.is_empty() {
        return per_machine(b, prefix);
    }
    if is_addition(b) {
        return None;
    }
    let name = format!("{prefix}{}", b.name);
    let has_rest = b.slots.iter().any(|s| s.rest);
    let has_default = b.slots.iter().any(|s| s.default.is_some());
    let simple = !has_rest && !has_default;
    let mut pre: Vec<String> = vec![];

    // The count check, with the sentence The Textsmith says.
    let text = bm::arg_text(b);
    let range = match (b.min, b.max) {
        (0, None) => None,
        (min, None) => Some(format!("(( $# >= {min} ))")),
        (0, Some(max)) => Some(format!("(( $# <= {max} ))")),
        (min, Some(max)) if min == max => Some(format!("(( $# == {min} ))")),
        (min, Some(max)) => Some(format!("(( $# >= {min} && $# <= {max} ))")),
    };
    if let Some(r) = range {
        pre.push(format!("{r} || {{ printf '%s\\n' {}\"$#.\" >&2; return 2; }}", sq(&format!("{} takes {text}; you gave ", b.name))));
    }

    // Which argument goes to which place.
    let mut one: Vec<Option<usize>> = vec![None; b.slots.len()];
    let fixed = b.slots.iter().filter(|s| !s.rest).count();
    if simple {
        for i in 0..b.slots.len() {
            one[i] = Some(i + 1);
        }
    } else if !has_rest {
        // Places with defaults: the argument if it was given, the default if not.
        for (i, s) in b.slots.iter().enumerate() {
            match &s.default {
                None => one[i] = Some(i + 1),
                Some(d) => {
                    let v = var(b, i);
                    // Another place it names: its argument, or its own default.
                    let d = default_words(b, d, |j| if b.slots[j].default.is_none() { format!("\"${{{}}}\"", j + 1) } else { format!("\"${{{}[@]}}\"", var(b, j)) });
                    pre.push(format!("local -a {v}=({d}); (( $# >= {} )) && {v}=(\"${{{}}}\")", i + 1, i + 1));
                }
            }
        }
    } else if b.slots.len() == 1 && b.slots[0].default.is_none() {
        // Only a place for the rest: all the arguments, "$@".
    } else {
        // A place for the rest takes what the other places leave over, where it stands.
        pre.push(format!("local _n=$(( $# > {fixed} ? $# - {fixed} : 0 )) _k=1"));
        for (i, s) in b.slots.iter().enumerate() {
            let v = var(b, i);
            let default = s.default.as_deref().map(|d| format!("({})", default_words(b, d, |j| format!("\"${{{}[@]}}\"", var(b, j))))).unwrap_or_else(|| "()".into());
            if s.rest {
                pre.push(format!("local -a {v}=(\"${{@:_k:_n}}\"); (( _k += _n )); (( _n )) || {v}={default}"));
            } else {
                pre.push(format!("local -a {v}={default}; (( _k <= $# )) && {{ {v}=(\"${{@:_k:1}}\"); (( _k++ )); }}"));
            }
        }
    }

    let mut body = String::new();
    let mut q = Quote::None;
    for p in &b.parts {
        match p {
            Part::Text(t) => {
                body.push_str(t);
                q = after(t, q);
            }
            Part::Slot(i) if b.slots.len() == 1 && b.slots[0].rest && b.slots[0].default.is_none() => body.push_str(match q {
                Quote::None => "\"$@\"",
                Quote::Double => "$*",
                Quote::Single => "'\"$*\"'",
            }),
            Part::Slot(i) => body.push_str(&use_at(q, one[*i], &var(b, *i))),
        }
    }
    // A count beyond the places: the arguments left over go at the end.
    let extra_allowed = b.max.map_or(true, |m| m > b.slots.len()) && !has_rest;
    if extra_allowed && (b.spec.is_some() || b.slots.is_empty()) && b.max != Some(0) {
        let from = fixed + 1;
        body = format!("{} {}", body.trim_end(), if from == 1 { "\"$@\"".to_string() } else { format!("\"${{@:{from}}}\"") });
    }

    let mut out = format!("# {}\n", bm::format(b).replace('\n', " "));
    if pre.is_empty() {
        out.push_str(&format!("{name}() {{ {}; }}\n", body.trim_end().trim_end_matches(';')));
    } else {
        out.push_str(&format!("{name}() {{\n"));
        for l in pre {
            out.push_str(&format!("  {l}\n"));
        }
        out.push_str(&format!("  {}\n}}\n", body.trim_end().trim_end_matches(';')));
    }
    Some(out)
}

/// Names that are already something in bash or zsh, or a program on PATH.
pub fn clash(name: &str) -> Option<String> {
    const WORDS: &[&str] = &[
        "alias", "bg", "bind", "break", "builtin", "caller", "case", "cd", "command", "compgen", "complete", "compopt", "continue", "coproc", "declare", "dirs", "disown", "do", "done", "echo", "elif", "else", "enable", "esac", "eval", "exec", "exit", "export", "false", "fc", "fg", "fi", "for", "function", "getopts", "hash", "help", "history", "if", "in", "jobs", "kill", "let", "local", "logout", "mapfile", "popd", "printf", "pushd", "pwd", "read", "readarray", "readonly", "return", "select", "set", "shift", "shopt", "source", "suspend", "test", "then", "time", "times", "trap", "true", "type", "typeset", "ulimit", "umask", "unalias", "unset", "until", "wait", "while",
        "autoload", "bindkey", "compdef", "emulate", "print", "setopt", "unsetopt", "whence", "where", "which", "zle", "zmodload", "zstyle", "repeat", "foreach", "end", "noglob", "nocorrect",
    ];
    if WORDS.contains(&name) {
        return Some("a shell builtin or keyword".into());
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if let Ok(m) = std::fs::metadata(&p) {
            use std::os::unix::fs::PermissionsExt;
            if m.is_file() && m.permissions().mode() & 0o111 != 0 {
                return Some(format!("a program, {}", p.display()));
            }
        }
    }
    None
}

/// The whole file. Warnings (names that clash, bookmarks left out) are returned apart.
pub fn export(list: &[Bookmark], prefix: &str, keep_clashes: bool) -> (String, Vec<String>) {
    let mut out = String::from(
        "# Bookmarks from The Textsmith, as shell functions for bash and zsh.\n\
         # Made by `textsmith bm export`; make it again after changing your bookmarks.\n\
         # Load it from ~/.bashrc or ~/.zshrc, for instance:\n\
         #   [ -f ~/.config/textsmith/bookmarks.sh ] && . ~/.config/textsmith/bookmarks.sh\n\n",
    );
    let mut warnings = vec![];
    if list.iter().any(|b| !b.name.is_empty() && !b.variants.is_empty()) {
        out.push_str(&machine_helper());
    }
    let mut seen = std::collections::HashSet::new();
    for b in list {
        if b.name.is_empty() {
            continue;
        }
        if !seen.insert(b.name.clone()) {
            warnings.push(format!("{}: a second bookmark with this name; only the first is exported.", b.name));
            continue;
        }
        match function(b, prefix) {
            None => warnings.push(format!("{}: left out; `{}` is an addition for #@, not a command on its own.", b.name, b.command)),
            Some(f) => {
                let name = format!("{prefix}{}", b.name);
                if let Some(what) = clash(&name) {
                    if !keep_clashes {
                        warnings.push(format!("{name}: left out; there is already {what}, which the function would hide. Rename the bookmark, or export with --prefix (or --keep-clashes)."));
                        continue;
                    }
                    warnings.push(format!("{name}: there is already {what}; the function hides it."));
                }
                out.push_str(&f);
                out.push('\n');
            }
        }
    }
    (out, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn f(line: &str) -> String {
        function(&bm::parse(line), "").unwrap()
    }

    #[test]
    fn plain_places_become_positionals() {
        assert_eq!(
            f("#site: rsync -rltvP --delete <source> forge:/var/www/thetextsmith/<destination>"),
            "# #site: rsync -rltvP --delete <source> forge:/var/www/thetextsmith/<destination>\nsite() {\n  (( $# == 2 )) || { printf '%s\\n' 'site takes 2 arguments: source, destination; you gave '\"$#.\" >&2; return 2; }\n  rsync -rltvP --delete \"${1}\" forge:/var/www/thetextsmith/\"${2}\"\n}\n"
        );
        assert_eq!(f("#vps: ssh forge"), "# #vps: ssh forge\nvps() {\n  (( $# <= 0 )) || { printf '%s\\n' 'vps takes no arguments; you gave '\"$#.\" >&2; return 2; }\n  ssh forge\n}\n");
    }

    #[test]
    fn places_inside_quotes() {
        assert!(f("#notes: find . -iname '*<>*'").contains("find . -iname '*'\"${1}\"'*'"));
        assert!(f("#say: echo \"hello <who> there\"").contains("echo \"hello ${1} there\""));
    }

    #[test]
    fn counts_pipes_and_additions() {
        assert!(f("#copy(2..): cp -v").contains("cp -v \"$@\""));
        assert!(f("#|count: wc -l").contains("  wc -l\n"));
        assert!(function(&bm::parse("#quiet: 2>/dev/null"), "").is_none());
        assert!(function(&bm::parse("ls -la"), "").is_none());
    }
}
