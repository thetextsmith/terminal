//! The review buffer's text: every command with its output, in order, in the
//! spirit of Emacs shell mode. Built from the blocks the shell marks gave.

#[derive(Debug, Clone, Default)]
pub struct Block {
    pub command: String,
    pub dir: String,
    pub prompt: String,
    pub start: (i64, i64),
    pub status: Option<i32>, // None while it runs
    pub text: String,
}

/// Where each command line starts, and whether that command failed.
#[derive(Debug, Clone, PartialEq)]
pub struct Heading {
    pub line: i32,
    pub lines: i32, // a command over several lines
    pub failed: bool,
}

pub fn status_text(status: Option<i32>) -> String {
    match status {
        None => " (running)".into(),
        Some(0) => String::new(),
        Some(130) => " (interrupted, exit 130)".into(),
        Some(n) => format!(" (failed, exit {n})"),
    }
}

/// Output as the terminal shows it, without trailing spaces or blank lines.
pub fn tidy(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    let end = lines.iter().rposition(|l| !l.is_empty()).map_or(0, |i| i + 1);
    lines[..end].join("\n")
}

/// The whole buffer. `live` gives the output so far of a command still running.
pub fn build(blocks: &[Block], format: &str, live: impl Fn(&Block) -> String) -> (String, Vec<Heading>) {
    let mut out = String::new();
    let mut heads = vec![];
    let mut line = 0i32;
    for b in blocks {
        let head = format
            .replace("{command}", &b.command)
            .replace("{status}", &status_text(b.status))
            .replace("{dir}", &b.dir)
            .replace("{prompt}", &b.prompt);
        let n = head.lines().count().max(1) as i32;
        heads.push(Heading { line, lines: n, failed: matches!(b.status, Some(s) if s != 0) });
        out.push_str(&head);
        out.push('\n');
        line += n;
        let text = if b.status.is_none() { tidy(&live(b)) } else { b.text.clone() };
        if !text.is_empty() {
            out.push_str(&text);
            out.push('\n');
            line += text.lines().count() as i32;
        }
    }
    (out, heads)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_and_output_in_order() {
        let blocks = vec![
            Block { command: "ls".into(), text: "a\nb".into(), status: Some(0), ..Default::default() },
            Block { command: "false".into(), text: String::new(), status: Some(1), ..Default::default() },
            Block { command: "echo \"x\ny\"".into(), text: "x\ny".into(), status: Some(0), ..Default::default() },
            Block { command: "sleep 9".into(), status: None, ..Default::default() },
        ];
        let (text, heads) = build(&blocks, "$ {command}{status}", |_| "so far  \n\n".into());
        assert_eq!(text, "$ ls\na\nb\n$ false (failed, exit 1)\n$ echo \"x\ny\"\nx\ny\n$ sleep 9 (running)\nso far\n");
        assert_eq!(heads.iter().map(|h| (h.line, h.failed)).collect::<Vec<_>>(), vec![(0, false), (3, true), (4, false), (8, false)]);
    }

    #[test]
    fn tidy_drops_trailing_blank_lines() {
        assert_eq!(tidy("a  \nb\n\n   \n"), "a\nb");
        assert_eq!(tidy("\n\n"), "");
    }
}
