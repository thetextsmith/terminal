//! Bookmarks: how a bookmark line is read, and how a call to one becomes a command.
//!
//! The same format as the Electron version (renderer/bookmarks.js), one per
//! line in the bookmarks file. A bookmark is typed as a comment, so the
//! shell ignores it if Enter is pressed by mistake. The label comes first and
//! ends at ": ":
//!
//! ```text
//! #vps: ssh forge                               "vps", no arguments
//! #sync music: rclone sync <> . --progress      "sync", one argument; "music" is a description
//! #notes: find . -type f -iname '*<>*'          one argument, inside the quotes
//! #copy: cp <src> <dst>                         named places: src, then dst
//! #burn: dd if=<iso> of=<disk:/dev/sdb>         disk has a default: 1 or 2 arguments
//! #copy(2): cp                                  exactly 2 arguments, added at the end
//! #copy(2..): cp -v   #l(..1): ls -l   #copy(1..3): cp      at least 2, at most 1, 1 to 3
//! #|count: wc -l                                | = piped into, for #@
//! ```
//!
//! A place with a default (`<disk:/dev/sdb>`, or `<:/dev/sdb>` without a
//! name) is still a place: an argument given for it is used, and when the
//! arguments run out before it, the default is filled in.
//!
//! `\<>` (or `\<name>`) is a literal `<>`. Older forms are still read:
//! `ssh forge#vps` and `ssh forge =vps`.

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub name: String,
    pub default: Option<String>,
    pub rest: bool, // <..> or <name..>: takes all the arguments left over
}

#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text(String),
    Slot(usize),
}

/// A count written in the label: `(2)`, `(2..)`, `(..1)`, `(1..3)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spec {
    pub min: usize,
    pub max: Option<usize>, // None: no upper limit
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bookmark {
    pub raw: String,
    pub command: String,
    pub name: String,
    pub desc: String,
    pub slots: Vec<Slot>,
    pub parts: Vec<Part>,
    pub min: usize,
    pub max: Option<usize>,
    pub spec: Option<Spec>,
    pub pipe: bool,
    /// A bookmark with a command per machine: [:anvil "...", :brainpower "..."].
    /// The fields above are those of this machine's command.
    pub variants: Vec<Variant>,
    pub machine: String,
    /// Set when no command is there for this machine: what to say instead.
    pub missing: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub machines: Vec<String>, // "*" stands for any other machine
    pub command: String,
}

/// The name of this machine, for bookmarks with a command per machine:
/// TEXTSMITH_MACHINE, else the machine file in the folder (kept out of dotfiles), else
/// /etc/hostname. Never from the bookmarks file: it travels between machines.
pub fn machine() -> String {
    static M: OnceLock<String> = OnceLock::new();
    M.get_or_init(|| {
        if let Ok(m) = std::env::var("TEXTSMITH_MACHINE") {
            if !m.trim().is_empty() {
                return m.trim().to_string();
            }
        }
        let first = |p: std::path::PathBuf| std::fs::read_to_string(p).ok().and_then(|t| t.lines().next().map(|l| l.trim().to_string())).filter(|l| !l.is_empty());
        if let Some(m) = first(crate::store::config_dir().join("machine")) {
            return m;
        }
        hostname()
    })
    .clone()
}

pub fn hostname() -> String {
    if let Ok(t) = std::fs::read_to_string("/etc/hostname") {
        if let Some(l) = t.lines().next().map(str::trim).filter(|l| !l.is_empty()) {
            return l.to_string();
        }
    }
    let mut buf = [0u8; 256];
    unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    String::from_utf8_lossy(&buf).trim_end_matches('\0').to_string()
}

/// `[:anvil "cmd", :brainpower :hammer 'cmd', :* "cmd"]`, or None when the
/// command is not written that way (a test such as `[ -f x ]` stays a command).
pub fn parse_variants(s: &str) -> Option<Vec<Variant>> {
    let s = s.trim();
    let inner = s.strip_prefix('[')?.strip_suffix(']')?;
    let c: Vec<char> = inner.chars().collect();
    let mut i = 0;
    let mut out = vec![];
    let mut machines: Vec<String> = vec![];
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() || ch == ',' {
            if ch == ',' && !machines.is_empty() {
                return None; // a name without a command
            }
            i += 1;
        } else if ch == ':' {
            i += 1;
            let start = i;
            while i < c.len() && (c[i].is_alphanumeric() || "_.-*".contains(c[i])) {
                i += 1;
            }
            if i == start {
                return None;
            }
            machines.push(c[start..i].iter().collect());
        } else if ch == '"' || ch == '\'' {
            if machines.is_empty() {
                return None;
            }
            i += 1;
            let mut cmd = String::new();
            loop {
                if i >= c.len() {
                    return None;
                }
                if c[i] == ch {
                    i += 1;
                    break;
                }
                if ch == '"' && c[i] == '\\' && i + 1 < c.len() && (c[i + 1] == '"' || c[i + 1] == '\\') {
                    cmd.push(c[i + 1]);
                    i += 2;
                    continue;
                }
                cmd.push(c[i]);
                i += 1;
            }
            out.push(Variant { machines: std::mem::take(&mut machines), command: cmd });
        } else {
            return None;
        }
    }
    if out.is_empty() || !machines.is_empty() {
        return None;
    }
    Some(out)
}

fn pick<'a>(vs: &'a [Variant], m: &str) -> Option<&'a Variant> {
    vs.iter()
        .find(|v| v.machines.iter().any(|x| x == m))
        .or_else(|| vs.iter().find(|v| v.machines.iter().any(|x| x == "*")))
}

fn machine_list(vs: &[Variant]) -> Vec<String> {
    vs.iter().flat_map(|v| v.machines.iter().cloned()).map(|m| if m == "*" { "any other machine".into() } else { m }).collect()
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).unwrap())
}

fn name_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"^([A-Za-z0-9_][A-Za-z0-9_.\-]*)(?::(\d*)(\.\.)?(\d*))?(?:\s+(.*))?$")
}
fn label_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"^#\s*(\|?)\s*([A-Za-z0-9_][A-Za-z0-9_.\-]*)(?:\((\d*)(\.\.)?(\d*)\))?((?:\s+[^:]*)?):\s+(\S.*)$")
}
/// A place (`<>`, `<name>`, `<name:default>`, `<:default>`), or an escaped one
/// that stands for itself.
fn place_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // A default may name another place: <to:<from>>.
    re(&R, r"\\(<(?:[A-Za-z_][A-Za-z0-9_\-]*)?(?:\.\.)?(?::(?:<[A-Za-z_][A-Za-z0-9_\-]*>|[^<>\n])*)?>)|<([A-Za-z_][A-Za-z0-9_\-]*)?(\.\.)?(?::((?:<[A-Za-z_][A-Za-z0-9_\-]*>|[^<>\n])*))?>")
}

/// `<name>` inside a default: another place's value.
pub(crate) fn ref_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"<([A-Za-z_][A-Za-z0-9_\-]*)>")
}

/// A default as it is said: `<from>` read as "from".
fn shown(d: &str) -> String {
    ref_re().replace_all(d, "$1").into_owned()
}

fn spec_of(lo: &str, range: bool, hi: &str) -> Option<Spec> {
    if lo.is_empty() && !range && hi.is_empty() {
        return None;
    }
    let n = |s: &str| s.parse::<usize>().unwrap_or(0);
    if !range {
        return Some(Spec { min: n(lo), max: Some(n(lo)) });
    }
    Some(Spec { min: n(lo), max: if hi.is_empty() { None } else { Some(n(hi)) } })
}

/// Walks a line the way the shell reads quotes, calling `f(index, char, prev)`
/// for each character outside quotes and escapes.
fn unquoted(line: &str, mut f: impl FnMut(usize, char, Option<char>)) {
    let mut q: Option<char> = None;
    let mut esc = false;
    let mut prev: Option<char> = None;
    for (i, c) in line.char_indices() {
        let p = prev;
        prev = Some(c);
        if esc {
            esc = false;
            continue;
        }
        if c == '\\' && q != Some('\'') {
            esc = true;
            continue;
        }
        if let Some(qc) = q {
            if c == qc {
                q = None;
            }
            continue;
        }
        if c == '\'' || c == '"' {
            q = Some(c);
            continue;
        }
        f(i, c, p);
    }
}

/// The position of the last # outside quotes.
pub fn last_hash(line: &str) -> Option<usize> {
    let mut last = None;
    unquoted(line, |i, c, _| {
        if c == '#' {
            last = Some(i)
        }
    });
    last
}

/// Positions of " =" outside quotes: where a name may start (older form).
fn eq_marks(line: &str) -> Vec<usize> {
    let mut out = vec![];
    unquoted(line, |i, c, p| {
        if c == '=' && i > 0 && p.map_or(false, |p| p.is_whitespace()) {
            out.push(i)
        }
    });
    out
}

pub fn parse(raw: &str) -> Bookmark {
    parse_on(raw, &machine())
}

/// A bookmark as it is on the machine `m`.
pub fn parse_on(raw: &str, m: &str) -> Bookmark {
    let raw = raw.trim();
    if let Some(l) = label_re().captures(raw) {
        let g = |i| l.get(i).map_or("", |m| m.as_str());
        let command = g(7).trim();
        let spec = spec_of(g(3), !g(4).is_empty(), g(5));
        let mut b = match parse_variants(command) {
            None => build(raw, command, g(2), g(6).trim(), spec),
            Some(vs) => {
                let chosen = pick(&vs, m).map(|v| v.command.clone());
                let mut b = build(raw, chosen.as_deref().unwrap_or(""), g(2), g(6).trim(), spec);
                if chosen.is_none() {
                    b.missing = Some(format!("{} has no command for this machine ({m}); it has commands for {}.", g(2), machine_list(&vs).join(", ")));
                }
                b.command = command.to_string();
                b.variants = vs;
                b
            }
        };
        b.machine = m.to_string();
        b.pipe = !g(1).is_empty();
        return b;
    }
    // A comment without a label: no name.
    let stripped = Regex::new(r"^#\s*").unwrap().replace(raw, "").to_string();
    let raw = stripped.as_str();
    let (mut command, mut name, mut desc, mut spec) = (raw.to_string(), String::new(), String::new(), None);
    // The name: after the last " =" whose remainder reads as a name, or (the
    // older form) after a final #.
    let mut at: Option<usize> = None;
    let mut m: Option<regex::Captures> = None;
    for i in eq_marks(raw).into_iter().rev() {
        if let Some(t) = name_re().captures(raw[i + 1..].trim()) {
            if !raw[..i].trim().is_empty() {
                at = Some(i);
                m = Some(t);
                break;
            }
        }
    }
    if at.is_none() {
        if let Some(h) = last_hash(raw) {
            if h > 0 {
                m = name_re().captures(raw[h + 1..].trim());
                at = m.as_ref().map(|_| h);
            }
        }
    }
    if let (Some(at), Some(m)) = (at, m) {
        let before = raw[..at].trim_end();
        if at > 0 && !before.is_empty() {
            let g = |i| m.get(i).map_or("", |x| x.as_str());
            command = before.to_string();
            name = g(1).to_string();
            desc = g(5).trim().to_string();
            spec = spec_of(g(2), !g(3).is_empty(), g(4));
        }
    }
    build(raw, &command, &name, &desc, spec)
}

fn build(raw: &str, command: &str, name: &str, desc: &str, spec: Option<Spec>) -> Bookmark {
    let mut slots: Vec<Slot> = vec![];
    let mut parts: Vec<Part> = vec![];
    let mut named: Vec<(String, usize)> = vec![];
    let mut last = 0;
    for c in place_re().captures_iter(command) {
        let whole = c.get(0).unwrap();
        if whole.start() > last {
            parts.push(Part::Text(command[last..whole.start()].to_string()));
        }
        last = whole.end();
        if let Some(lit) = c.get(1) {
            parts.push(Part::Text(lit.as_str().to_string())); // \<> stays as <>
            continue;
        }
        let pname = c.get(2).map_or("", |m| m.as_str());
        let rest = c.get(3).is_some();
        let default = c.get(4).map(|m| m.as_str().to_string());
        // A name used again is the same place; so is a second <..>.
        let found = if !pname.is_empty() {
            named.iter().find(|(n, _)| n == pname).map(|x| x.1)
        } else if rest {
            slots.iter().position(|s| s.rest && s.name.is_empty())
        } else {
            None
        };
        let slot = match found {
            Some(s) => {
                if slots[s].default.is_none() && default.is_some() {
                    slots[s].default = default;
                }
                s
            }
            None => {
                if rest && slots.iter().any(|s| s.rest) {
                    // Only one place can take what is left over; a second one is literal text.
                    parts.push(Part::Text(whole.as_str().to_string()));
                    continue;
                }
                slots.push(Slot { name: pname.to_string(), default, rest });
                if !pname.is_empty() {
                    named.push((pname.to_string(), slots.len() - 1));
                }
                slots.len() - 1
            }
        };
        parts.push(Part::Slot(slot));
    }
    if last < command.len() {
        parts.push(Part::Text(command[last..].to_string()));
    }
    // Without a count, every place up to the last one without a default must
    // be given; the ones after it may be left to their defaults.
    // A place for the rest takes any number, none included, so it is never required.
    let fixed: Vec<&Slot> = slots.iter().filter(|s| !s.rest).collect();
    let required = fixed.iter().rposition(|s| s.default.is_none()).map_or(0, |i| i + 1);
    let has_rest = slots.iter().any(|s| s.rest);
    let (min, max) = match spec {
        Some(s) => (s.min, s.max.map(|m| m.max(s.min))),
        None => (required, if has_rest { None } else { Some(slots.len()) }),
    };
    Bookmark {
        raw: raw.to_string(),
        command: command.to_string(),
        name: name.to_string(),
        desc: desc.to_string(),
        slots,
        parts,
        min,
        max,
        spec,
        pipe: false,
        variants: vec![],
        machine: String::new(),
        missing: None,
    }
}

impl Bookmark {
    /// The command that runs on this machine, places and all.
    pub fn this_command(&self) -> String {
        if self.variants.is_empty() {
            return self.command.clone();
        }
        pick(&self.variants, &self.machine).map(|v| v.command.clone()).unwrap_or_default()
    }
}

impl Bookmark {
    /// Arguments are expected: some are required, or a place takes any number.
    pub fn wants_args(&self) -> bool {
        self.min > 0 || self.slots.iter().any(|s| s.rest)
    }
}

/// A bookmark as it is written and stored: "#name(count) description: command",
/// or just the command when it has no name.
pub fn format(b: &Bookmark) -> String {
    if b.name.is_empty() {
        return b.command.clone();
    }
    let count = match b.spec {
        None => String::new(),
        Some(Spec { min, max }) if Some(min) == max => format!("({min})"),
        Some(Spec { min, max }) => format!(
            "({}..{})",
            if min == 0 { String::new() } else { min.to_string() },
            max.map_or(String::new(), |m| m.to_string())
        ),
    };
    format!(
        "#{}{}{}{}: {}",
        if b.pipe { "|" } else { "" },
        b.name,
        count,
        if b.desc.is_empty() { String::new() } else { format!(" {}", b.desc) },
        b.command
    )
}

/// Split what was typed into words, keeping each word exactly as typed.
pub fn tokenize(s: &str) -> Vec<String> {
    let mut out = vec![];
    let (mut cur, mut q, mut esc, mut in_tok) = (String::new(), None::<char>, false, false);
    for c in s.chars() {
        if esc {
            cur.push(c);
            esc = false;
            continue;
        }
        if c == '\\' && q != Some('\'') {
            cur.push(c);
            esc = true;
            in_tok = true;
            continue;
        }
        if let Some(qc) = q {
            cur.push(c);
            if c == qc {
                q = None;
            }
            continue;
        }
        if c == '\'' || c == '"' {
            q = Some(c);
            cur.push(c);
            in_tok = true;
            continue;
        }
        if c.is_whitespace() {
            if in_tok {
                out.push(std::mem::take(&mut cur));
                in_tok = false;
            }
            continue;
        }
        cur.push(c);
        in_tok = true;
    }
    if in_tok {
        out.push(cur);
    }
    out
}

fn plural(n: usize, w: &str) -> String {
    format!("{n} {w}{}", if n == 1 { "" } else { "s" })
}

/// How many arguments a bookmark takes, and the names of its places, in words.
pub fn arg_text(b: &Bookmark) -> String {
    let t = match (b.min, b.max) {
        (_, Some(0)) => "no arguments".to_string(),
        (min, Some(max)) if min == max => plural(min, "argument"),
        (0, None) => "any number of arguments".to_string(),
        (min, None) => format!("at least {}", plural(min, "argument")),
        (0, Some(max)) => format!("up to {}", plural(max, "argument")),
        (min, Some(max)) => format!("{min} to {max} arguments"),
    };
    let mut names = vec![];
    for (i, s) in b.slots.iter().enumerate() {
        let name = match (&s.name[..], s.rest) {
            ("", true) => "the rest".to_string(),
            ("", false) => String::new(),
            (n, true) => format!("{n}, any number"),
            (n, false) => n.to_string(),
        };
        match (name.is_empty(), &s.default) {
            (true, None) => {}
            (true, Some(d)) => names.push(format!("argument {} defaults to {}", i + 1, shown(d))),
            (false, None) if s.rest && s.name.is_empty() && b.slots.len() == 1 => {}
            (false, None) => names.push(name),
            (false, Some(d)) if s.rest => names.push(format!("{name}, defaulting to {}", shown(d))),
            (false, Some(d)) => names.push(format!("{name} defaults to {}", shown(d))),
        }
    }
    if names.is_empty() {
        t
    } else {
        format!("{t}: {}", names.join(", "))
    }
}

/// What a screen reader says about a bookmark: name, description, arguments.
pub fn describe(b: &Bookmark) -> String {
    let first = if b.name.is_empty() { b.command.clone() } else { b.name.clone() };
    let mut v = vec![first];
    if b.pipe {
        v.push("piped into".into());
    }
    if !b.desc.is_empty() {
        v.push(b.desc.clone());
    }
    if !b.variants.is_empty() {
        v.push(format!("for {}", machine_list(&b.variants).join(", ")));
        if b.missing.is_some() {
            v.push(format!("none for this machine ({})", b.machine));
            return v.join(". ");
        }
    }
    v.push(arg_text(b));
    v.join(". ")
}

/// Fill in a bookmark with the arguments given.
pub fn expand(b: &Bookmark, args: &[String]) -> Result<String, String> {
    if let Some(e) = &b.missing {
        return Err(e.clone());
    }
    let n = args.len();
    let who = if b.name.is_empty() { "This bookmark" } else { &b.name };
    if n < b.min || b.max.map_or(false, |m| n > m) {
        return Err(format!("{who} takes {}; you gave {n}.", arg_text(b)));
    }
    // Which arguments go to which place. A place for the rest takes what the
    // other places leave over, at its own position.
    let fixed = b.slots.iter().filter(|s| !s.rest).count();
    let rest_n = n.saturating_sub(fixed);
    let mut values: Vec<Option<String>> = vec![None; b.slots.len()];
    let mut k = 0;
    for (i, s) in b.slots.iter().enumerate() {
        if s.rest {
            if rest_n > 0 {
                values[i] = Some(args[k..k + rest_n].join(" "));
                k += rest_n;
            }
        } else if k < n {
            values[i] = Some(args[k].clone());
            k += 1;
        }
    }
    for (i, s) in b.slots.iter().enumerate() {
        if values[i].is_none() && s.default.is_none() && !s.rest {
            let what = if s.name.is_empty() { format!("argument {}", i + 1) } else { format!("<{}>", s.name) };
            return Err(format!("{who} is missing {what}."));
        }
    }
    // A default that names another place (<to:<from>>) takes that place's value.
    let given = |j: usize| values[j].clone().unwrap_or_else(|| b.slots[j].default.clone().unwrap_or_default());
    let value = |i: usize| -> String {
        if let Some(v) = &values[i] {
            return v.clone();
        }
        let d = b.slots[i].default.clone().unwrap_or_default();
        ref_re()
            .replace_all(&d, |c: &regex::Captures| match b.slots.iter().position(|s| s.name == c[1]) {
                Some(j) if j != i => given(j),
                _ => c[0].to_string(),
            })
            .into_owned()
    };
    let mut command: String = b
        .parts
        .iter()
        .map(|p| match p {
            Part::Text(t) => t.clone(),
            Part::Slot(i) => value(*i),
        })
        .collect();
    if k < n {
        command = format!("{} {}", command.trim_end(), args[k..].join(" "));
    }
    Ok(command)
}

/// "command #@name args @other args": the position of #@ (outside quotes, at
/// the start or after a space).
pub fn inline_at(line: &str) -> Option<usize> {
    let mut found = None;
    let bytes = line.as_bytes();
    unquoted(line, |i, c, p| {
        if found.is_none() && c == '#' && bytes.get(i + 1) == Some(&b'@') && (i == 0 || p.map_or(false, |p| p.is_whitespace())) {
            found = Some(i)
        }
    });
    found
}

/// Expand the #@ calls at the end of a line. `find` gives a bookmark by name.
/// None when the line has no #@.
pub fn expand_inline<'a>(line: &str, find: impl Fn(&str) -> Option<&'a Bookmark>) -> Option<Result<String, String>> {
    let at = inline_at(line)?;
    let mut command = line[..at].trim_end().to_string();
    let mut calls: Vec<(String, Vec<String>)> = vec![];
    for w in tokenize(&line[at + 1..]) {
        if let Some(name) = w.strip_prefix('@') {
            calls.push((name.to_string(), vec![]));
        } else if let Some(c) = calls.last_mut() {
            c.1.push(w);
        }
    }
    for (name, args) in calls {
        if name.is_empty() {
            return Some(Err("A bookmark name is missing after @.".into()));
        }
        let Some(b) = find(&name) else { return Some(Err(format!("No bookmark called {name}."))) };
        match expand(b, &args) {
            Err(e) => return Some(Err(e)),
            Ok(r) => {
                command = if command.is_empty() { r } else { format!("{command}{}{r}", if b.pipe { " | " } else { " " }) };
            }
        }
    }
    Some(Ok(command))
}

/// A bookmark by name: exactly, or else ignoring case.
pub fn find<'a>(list: &'a [Bookmark], name: &str) -> Option<&'a Bookmark> {
    list.iter()
        .find(|b| !b.name.is_empty() && b.name == name)
        .or_else(|| list.iter().find(|b| !b.name.is_empty() && b.name.eq_ignore_ascii_case(name)))
}

/// A line typed as a call: "name args...", maybe followed by #@ additions.
pub fn call(list: &[Bookmark], line: &str) -> Result<String, String> {
    let line = line.trim();
    let (head, tail) = match inline_at(line) {
        Some(0) => return expand_inline(line, |n| find(list, n)).unwrap(),
        Some(i) => (&line[..i], Some(&line[i..])),
        None => (line, None),
    };
    let words = tokenize(head);
    let Some(first) = words.first() else { return Err("Type a bookmark name and its arguments.".into()) };
    let name = first.trim_start_matches("#@").trim_start_matches('@');
    let b = find(list, name).ok_or_else(|| format!("No bookmark called {name}."))?;
    let cmd = expand(b, &words[1..])?;
    match tail {
        None => Ok(cmd),
        Some(t) => expand_inline(&format!("{cmd} {t}"), |n| find(list, n)).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(line: &str, args: &[&str]) -> Result<String, String> {
        expand(&parse(line), &args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }
    fn ok(s: &str) -> Result<String, String> {
        Ok(s.to_string())
    }

    #[test]
    fn name_after_hash_with_or_without_space() {
        assert_eq!((parse("ssh forge#vps").command, parse("ssh forge#vps").name), ("ssh forge".into(), "vps".into()));
        assert_eq!((parse("ssh forge #vps").command, parse("ssh forge #vps").name), ("ssh forge".into(), "vps".into()));
        assert_eq!(parse("ssh forge").name, "");
    }

    #[test]
    fn words_after_the_name_are_a_description() {
        let b = parse("rclone sync <> . --progress#sync music");
        assert_eq!(b.name, "sync");
        assert_eq!(b.desc, "music");
        assert_eq!(describe(&b), "sync. music. 1 argument");
    }

    #[test]
    fn hash_inside_quotes_or_param_length_is_not_a_name() {
        assert_eq!(parse("echo '#nope'").name, "");
        assert_eq!(parse("echo ${#x}").name, "");
        assert_eq!(parse("grep '#todo' notes.md#todos").name, "todos");
    }

    #[test]
    fn anonymous_places_fill_in_order_even_in_quotes() {
        assert_eq!(run("rclone sync <> . --progress#sync", &["drive:Music"]), ok("rclone sync drive:Music . --progress"));
        assert_eq!(run("find . -type f -iname '*<>*'#notes", &["artificial"]), ok("find . -type f -iname '*artificial*'"));
    }

    #[test]
    fn named_places_take_arguments_in_order_and_repeat() {
        assert_eq!(run("cp <src> <dst> && ls -l <dst>#copy", &["a.txt", "b/"]), ok("cp a.txt b/ && ls -l b/"));
        assert_eq!(arg_text(&parse("cp <src> <dst>#copy")), "2 arguments: src, dst");
    }

    #[test]
    fn count_without_places_adds_arguments_at_the_end() {
        assert_eq!(run("cp#copy:2", &["a", "b"]), ok("cp a b"));
        assert_eq!(run("cp -v#copy:2..", &["a", "b", "c/"]), ok("cp -v a b c/"));
        assert_eq!(run("ls#l:..1", &[]), ok("ls"));
        assert_eq!(run("ls#l:..1", &["/tmp"]), ok("ls /tmp"));
    }

    #[test]
    fn wrong_number_of_arguments_is_explained() {
        assert_eq!(run("rclone sync <> .#sync", &[]).unwrap_err(), "sync takes 1 argument; you gave 0.");
        assert_eq!(run("cp#copy:2..", &["a"]).unwrap_err(), "copy takes at least 2 arguments; you gave 1.");
        assert_eq!(run("ls#l:..1", &["a", "b"]).unwrap_err(), "l takes up to 1 argument; you gave 2.");
        assert_eq!(run("ssh forge#vps", &["x"]).unwrap_err(), "vps takes no arguments; you gave 1.");
        assert_eq!(run("cp <src> <dst>#copy:1..", &["a"]).unwrap_err(), "copy is missing <dst>.");
    }

    #[test]
    fn arguments_keep_their_quotes() {
        assert_eq!(tokenize(r#"copy "my file.txt" it\'s 'a b'"#), vec!["copy", "\"my file.txt\"", "it\\'s", "'a b'"]);
        assert_eq!(run("cp <> <>#copy", &["\"my file.txt\"", "b/"]), ok("cp \"my file.txt\" b/"));
    }

    #[test]
    fn label_form() {
        let b = parse(r#"#organise txt: mkdir -p <txt> && find . -type f -name "*.<txt>" -exec mv {} <txt>/ +"#);
        assert_eq!(b.name, "organise");
        assert_eq!(b.desc, "txt");
        assert_eq!(expand(&b, &["txt".into()]), ok(r#"mkdir -p txt && find . -type f -name "*.txt" -exec mv {} txt/ +"#));
        assert_eq!((parse("#vps: ssh forge").command, parse("#vps: ssh forge").name), ("ssh forge".into(), "vps".into()));
        assert_eq!(run("#copy(2): cp", &["a", "b"]), ok("cp a b"));
        assert_eq!(arg_text(&parse("#copy(2..): cp -v")), "at least 2 arguments");
        assert_eq!(arg_text(&parse("#l(..1): ls -l")), "up to 1 argument");
    }

    #[test]
    fn only_the_label_is_read() {
        let b = parse("#burn: dd if=<iso> of=/dev/sdb bs=1M status=progress");
        assert_eq!(b.name, "burn");
        assert_eq!(b.command, "dd if=<iso> of=/dev/sdb bs=1M status=progress");
        assert_eq!(parse(r#"#same: [ "$a" = "$b" ] && echo same &"#).command, r#"[ "$a" = "$b" ] && echo same &"#);
        assert_eq!(parse("#web: curl https://example.com/a#top").command, "curl https://example.com/a#top");
        assert_eq!(parse(r#"[ "$a" = "$b" ] && echo same"#).name, "");
        assert_eq!(parse("#ssh forge").name, "");
        assert_eq!(parse("#ssh forge").command, "ssh forge");
    }

    #[test]
    fn format_writes_the_label_form_back() {
        for line in ["#vps: ssh forge", "#sync music: rclone sync <> .", "#copy(2): cp", "#copy(2..): cp -v", "#l(..1): ls", "#c(1..3): cp", "#burn: dd if=<iso> of=<disk:/dev/sdb>"] {
            assert_eq!(format(&parse(line)), line);
        }
        assert_eq!(format(&parse("ssh forge#vps")), "#vps: ssh forge");
        assert_eq!(format(&parse("ssh forge =vps")), "#vps: ssh forge");
        assert_eq!(format(&parse("ls -la")), "ls -la");
    }

    #[test]
    fn escaped_place_is_literal() {
        let b = parse(r"#open: exec 3\<>/dev/tcp/<host>/80");
        assert_eq!(b.slots.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["host"]);
        assert_eq!(expand(&b, &["example.com".into()]), ok("exec 3<>/dev/tcp/example.com/80"));
        assert_eq!(parse(r"#e: echo \<html\>").slots.len(), 0);
        assert_eq!(expand(&parse(r"#e: echo \<a:b>"), &[]), ok("echo <a:b>"));
    }

    #[test]
    fn inline_calls_like_global_aliases() {
        let marks: Vec<Bookmark> = ["#|count: wc -l", "#|sorted: sort", "#|grab: grep -i <>", "#quiet: 2>/dev/null", "#vps: ssh forge"].iter().map(|l| parse(l)).collect();
        let f = |n: &str| find(&marks, n);
        assert_eq!(format(&marks[0]), "#|count: wc -l");
        assert_eq!(describe(&marks[0]), "count. piped into. no arguments");
        assert_eq!(expand_inline("ls -1F #@count", f), Some(ok("ls -1F | wc -l")));
        assert_eq!(expand_inline("find . -type f #@grab TODO @sorted @count", f), Some(ok("find . -type f | grep -i TODO | sort | wc -l")));
        assert_eq!(expand_inline("make #@quiet", f), Some(ok("make 2>/dev/null")));
        assert_eq!(expand_inline("#@vps", f), Some(ok("ssh forge")));
        assert_eq!(expand_inline("ls #@nope", f), Some(Err("No bookmark called nope.".into())));
        assert_eq!(expand_inline("ls #@grab", f), Some(Err("grab takes 1 argument; you gave 0.".into())));
        assert_eq!(expand_inline("echo \"#@count\"", f), None);
        assert_eq!(expand_inline("echo a#@count", f), None);
        assert_eq!(expand_inline("ls -la", f), None);
    }

    // ---- default values ----

    #[test]
    fn a_default_is_filled_in_when_left_out() {
        let b = parse("#burn: dd if=<iso> of=<disk:/dev/sdb> bs=1M status=progress");
        assert_eq!(b.slots, vec![Slot { name: "iso".into(), default: None, rest: false }, Slot { name: "disk".into(), default: Some("/dev/sdb".into()), rest: false }]);
        assert_eq!((b.min, b.max), (1, Some(2)));
        assert_eq!(expand(&b, &["a.iso".into()]), ok("dd if=a.iso of=/dev/sdb bs=1M status=progress"));
        assert_eq!(expand(&b, &["a.iso".into(), "/dev/sdc".into()]), ok("dd if=a.iso of=/dev/sdc bs=1M status=progress"));
        assert_eq!(expand(&b, &[]).unwrap_err(), "burn takes 1 to 2 arguments: iso, disk defaults to /dev/sdb; you gave 0.");
        assert_eq!(describe(&b), "burn. 1 to 2 arguments: iso, disk defaults to /dev/sdb");
    }

    #[test]
    fn the_example_from_the_request() {
        let b = parse("#dd: dd if=<iso> of=<disk:/dev/sdb>");
        assert_eq!(expand(&b, &["x.iso".into()]), ok("dd if=x.iso of=/dev/sdb"));
    }

    #[test]
    fn defaults_repeat_with_their_name_and_may_be_given_later() {
        let b = parse("#mnt: mount <dev> <dir:/mnt> && ls <dir>");
        assert_eq!(expand(&b, &["/dev/sdb1".into()]), ok("mount /dev/sdb1 /mnt && ls /mnt"));
        let b = parse("#m2: echo <dir> then <dir:/mnt>");
        assert_eq!(expand(&b, &[]), ok("echo /mnt then /mnt"));
        assert_eq!((b.min, b.max), (0, Some(1)));
    }

    #[test]
    fn all_defaults_means_no_argument_is_needed() {
        let b = parse("#serve: python -m http.server <port:8000> --bind <addr:127.0.0.1>");
        assert_eq!(arg_text(&b), "up to 2 arguments: port defaults to 8000, addr defaults to 127.0.0.1");
        assert_eq!(expand(&b, &[]), ok("python -m http.server 8000 --bind 127.0.0.1"));
        assert_eq!(expand(&b, &["9000".into()]), ok("python -m http.server 9000 --bind 127.0.0.1"));
    }

    #[test]
    fn a_default_before_a_required_place_is_only_used_with_a_count() {
        // Arguments fill places in order, so a later required place makes the earlier one required too.
        let b = parse("#cp2: cp <src:a.txt> <dst>");
        assert_eq!((b.min, b.max), (2, Some(2)));
        let b = parse("#cp3(1..2): cp <src:a.txt> <dst:b/>");
        assert_eq!(expand(&b, &["x".into()]), ok("cp x b/"));
        let b = parse("#cp4(0..2): cp <src:a.txt> <dst>");
        assert_eq!(expand(&b, &["x".into()]).unwrap_err(), "cp4 is missing <dst>.");
    }

    #[test]
    fn anonymous_places_can_have_defaults_too() {
        let b = parse("#ping: ping -c <:3> <host>");
        assert_eq!(expand(&b, &["4".into(), "forge".into()]), ok("ping -c 4 forge"));
        let b = parse("#up: ping -c 3 <host:forge> <:-q>");
        assert_eq!(expand(&b, &[]), ok("ping -c 3 forge -q"));
        assert_eq!(arg_text(&b), "up to 2 arguments: host defaults to forge, argument 2 defaults to -q");
    }

    #[test]
    fn defaults_work_in_inline_calls_and_calls() {
        let marks: Vec<Bookmark> = ["#|head: head -n <n:10>", "#burn: dd if=<iso> of=<disk:/dev/sdb>", "#|count: wc -l"].iter().map(|l| parse(l)).collect();
        assert_eq!(expand_inline("ls #@head", |n| find(&marks, n)), Some(ok("ls | head -n 10")));
        assert_eq!(expand_inline("ls #@head 3 @count", |n| find(&marks, n)), Some(ok("ls | head -n 3 | wc -l")));
        assert_eq!(call(&marks, "burn arch.iso"), ok("dd if=arch.iso of=/dev/sdb"));
        assert_eq!(call(&marks, "Burn arch.iso /dev/sdc"), ok("dd if=arch.iso of=/dev/sdc"));
        assert_eq!(call(&marks, "#@burn arch.iso"), ok("dd if=arch.iso of=/dev/sdb"));
        assert_eq!(call(&marks, "nope").unwrap_err(), "No bookmark called nope.");
        assert_eq!(call(&marks, "burn x.iso #@count"), ok("dd if=x.iso of=/dev/sdb | wc -l"));
        assert_eq!(call(&marks, "  ").unwrap_err(), "Type a bookmark name and its arguments.");
    }

    // ---- <..>: any number of arguments ----

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn rest_takes_any_number() {
        let b = parse("#install: sudo apt update && sudo apt install -y <..>");
        assert_eq!((b.min, b.max), (0, None));
        assert_eq!(arg_text(&b), "any number of arguments");
        assert_eq!(expand(&b, &args(&["vim"])), ok("sudo apt update && sudo apt install -y vim"));
        assert_eq!(expand(&b, &args(&["vim", "git", "\"my pkg\""])), ok("sudo apt update && sudo apt install -y vim git \"my pkg\""));
        assert_eq!(expand(&b, &[]), ok("sudo apt update && sudo apt install -y "));
        assert_eq!(format(&b), "#install: sudo apt update && sudo apt install -y <..>");
    }

    #[test]
    fn rest_in_the_middle_with_fixed_places() {
        let b = parse("#cpto: cp -v <..> <dst> && ls <dst>");
        assert_eq!((b.min, b.max), (1, None));
        assert_eq!(arg_text(&b), "at least 1 argument: the rest, dst");
        assert_eq!(expand(&b, &args(&["a", "b", "c/"])), ok("cp -v a b c/ && ls c/"));
        assert_eq!(expand(&b, &args(&["c/"])), ok("cp -v  c/ && ls c/"));
        assert_eq!(expand(&b, &[]).unwrap_err(), "cpto takes at least 1 argument: the rest, dst; you gave 0.");
    }

    #[test]
    fn named_rest_repeats_and_can_have_a_default() {
        let b = parse("#pk: echo installing <pkgs..> && apt install <pkgs..>");
        assert_eq!(expand(&b, &args(&["a", "b"])), ok("echo installing a b && apt install a b"));
        assert_eq!(arg_text(&b), "any number of arguments: pkgs, any number");
        let b = parse("#lint: cargo clippy <opts..:--all-targets>");
        assert_eq!(expand(&b, &[]), ok("cargo clippy --all-targets"));
        assert_eq!(expand(&b, &args(&["-q", "--tests"])), ok("cargo clippy -q --tests"));
        assert_eq!(arg_text(&b), "any number of arguments: opts, any number, defaulting to --all-targets");
        let b = parse("#x: echo <a> <..>");
        assert_eq!(expand(&b, &args(&["1", "2", "3"])), ok("echo 1 2 3"));
        assert_eq!(expand(&b, &[]).unwrap_err(), "x takes at least 1 argument: a, the rest; you gave 0.");
    }

    #[test]
    fn rest_with_a_count_and_in_calls() {
        let b = parse("#install(1..): sudo apt install -y <..>");
        assert_eq!(expand(&b, &[]).unwrap_err(), "install takes at least 1 argument; you gave 0.");
        let marks: Vec<Bookmark> = ["#install: sudo apt install -y <..>", "#|grep: grep -E <..>"].iter().map(|l| parse(l)).collect();
        assert_eq!(call(&marks, "install vim git htop"), ok("sudo apt install -y vim git htop"));
        assert_eq!(expand_inline("ls #@grep -i foo", |n| find(&marks, n)), Some(ok("ls | grep -E -i foo")));
        assert_eq!(expand(&parse(r"#e: echo \<..>"), &[]), ok("echo <..>"));
    }

    // ---- a command per machine ----

    const UPDATE: &str = r#"#update: [:anvil "sudo apt update -y && sudo apt upgrade -y", :brainpower "sudo pacman -Syu --noconfirm", :hammer "sudo dnf update"]"#;

    #[test]
    fn each_machine_gets_its_own_command() {
        assert_eq!(expand(&parse_on(UPDATE, "brainpower"), &[]), ok("sudo pacman -Syu --noconfirm"));
        assert_eq!(expand(&parse_on(UPDATE, "anvil"), &[]), ok("sudo apt update -y && sudo apt upgrade -y"));
        assert_eq!(expand(&parse_on(UPDATE, "hammer"), &[]), ok("sudo dnf update"));
        assert!(expand(&parse_on(UPDATE, "Hammer"), &[]).is_err()); // names are exact, as in the exported functions
        assert_eq!(
            expand(&parse_on(UPDATE, "vm"), &[]).unwrap_err(),
            "update has no command for this machine (vm); it has commands for anvil, brainpower, hammer."
        );
        assert_eq!(format(&parse_on(UPDATE, "vm")), UPDATE);
        assert_eq!(describe(&parse_on(UPDATE, "anvil")), "update. for anvil, brainpower, hammer. no arguments");
        assert_eq!(describe(&parse_on(UPDATE, "vm")), "update. for anvil, brainpower, hammer. none for this machine (vm)");
    }

    #[test]
    fn several_names_a_fallback_places_and_quotes() {
        let l = r#"#up: [:anvil :forge 'sudo apt install -y <..>', :* "echo \"no apt here\" <..>"]"#;
        assert_eq!(expand(&parse_on(l, "forge"), &args(&["vim", "git"])), ok("sudo apt install -y vim git"));
        assert_eq!(expand(&parse_on(l, "other"), &args(&["x"])), ok("echo \"no apt here\" x"));
        let l = "#burn(1..2): [:anvil \"dd if=<iso> of=<disk:/dev/sdb>\", :brainpower \"dd if=<iso> of=<disk:/dev/sdc>\"]";
        assert_eq!(expand(&parse_on(l, "brainpower"), &args(&["a.iso"])), ok("dd if=a.iso of=/dev/sdc"));
        assert_eq!(arg_text(&parse_on(l, "brainpower")), "1 to 2 arguments: iso, disk defaults to /dev/sdc");
    }

    #[test]
    fn brackets_that_are_not_a_list_stay_a_command() {
        assert_eq!(parse_on("#t: [ -f x ]", "a").variants.len(), 0);
        assert_eq!(expand(&parse_on("#t: [ -f x ]", "a"), &[]), ok("[ -f x ]"));
        assert_eq!(parse_on("#t: [ \"$a\" = \"$b\" ]", "a").variants.len(), 0);
        assert!(parse_variants("[:a]").is_none());
        assert!(parse_variants("[\"x\"]").is_none());
        assert!(parse_variants("[:a \"x\", :b]").is_none());
        assert_eq!(parse_variants("[:a:b 'x' :c \"y\"]").unwrap().len(), 2);
    }

    // ---- a default that is another place ----

    #[test]
    fn a_default_can_be_another_place() {
        let b = parse("#send: rclone sync <from> drive:Library/<to:<from>> --progress");
        assert_eq!((b.min, b.max), (1, Some(2)));
        assert_eq!(arg_text(&b), "1 to 2 arguments: from, to defaults to from");
        assert_eq!(expand(&b, &args(&["Music"])), ok("rclone sync Music drive:Library/Music --progress"));
        assert_eq!(expand(&b, &args(&["Music", "Songs"])), ok("rclone sync Music drive:Library/Songs --progress"));
        assert_eq!(format(&b), "#send: rclone sync <from> drive:Library/<to:<from>> --progress");
        // mixed with text, and with a place that itself has a default
        let b = parse("#bk: cp <f> <to:<f>.bak>");
        assert_eq!(expand(&b, &args(&["a.txt"])), ok("cp a.txt a.txt.bak"));
        let b = parse("#d: echo <a:x> <b:<a>-y>");
        assert_eq!(expand(&b, &[]), ok("echo x x-y"));
        assert_eq!(expand(&b, &args(&["q"])), ok("echo q q-y"));
        // a name that is no place stays as written
        assert_eq!(expand(&parse("#n: echo <a> <b:<zz>>"), &args(&["1"])), ok("echo 1 <zz>"));
        assert_eq!(expand(&parse(r"#e: echo \<b:<a>>"), &[]), ok("echo <b:<a>>"));
    }

    #[test]
    fn a_default_may_hold_spaces_and_quotes() {
        let b = parse("#g: grep -r <pat:'TODO|FIXME'> <dir:.>");
        assert_eq!(expand(&b, &[]), ok("grep -r 'TODO|FIXME' ."));
    }
}
