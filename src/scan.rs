//! Picks the shell integration's marks out of what the shell writes.
//! Everything else goes on to the terminal untouched.

#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    Prompt,          // 133;A  the prompt is about to be shown
    InputStart,      // 133;B  the prompt is shown: typing starts here
    Output,          // 133;C  the command's output starts
    Done(i32),       // 133;D;status
    Cwd(String),     // 7      file://host/path
    PromptText(String), // 7772  the prompt, for the title
    Line(String),    // 7773  the line Enter was pressed on
}

#[derive(Debug, PartialEq)]
pub enum Piece {
    Text(Vec<u8>),
    Mark(Mark),
}

#[derive(Default)]
pub struct Scanner {
    carry: Vec<u8>,
}

const MAX_CARRY: usize = 1 << 16;

impl Scanner {
    pub fn push(&mut self, data: &[u8]) -> Vec<Piece> {
        let mut buf = std::mem::take(&mut self.carry);
        buf.extend_from_slice(data);
        let mut out: Vec<Piece> = vec![];
        let mut text_start = 0;
        let mut i = 0;
        let n = buf.len();
        while i < n {
            if buf[i] != 0x1b {
                i += 1;
                continue;
            }
            if i + 1 >= n {
                // A lone ESC at the end: it may start a mark.
                if n - i < MAX_CARRY {
                    flush(&mut out, &buf[text_start..i]);
                    self.carry = buf[i..].to_vec();
                    return out;
                }
                break;
            }
            if buf[i + 1] != b']' {
                i += 1;
                continue;
            }
            // An OSC: find its end, BEL or ESC \.
            let body_start = i + 2;
            let mut j = body_start;
            let mut end = None;
            while j < n {
                if buf[j] == 0x07 {
                    end = Some((j, j + 1));
                    break;
                }
                if buf[j] == 0x1b && j + 1 < n && buf[j + 1] == b'\\' {
                    end = Some((j, j + 2));
                    break;
                }
                if buf[j] == 0x1b && j + 1 >= n {
                    break;
                }
                j += 1;
            }
            let Some((body_end, after)) = end else {
                if n - i < MAX_CARRY {
                    flush(&mut out, &buf[text_start..i]);
                    self.carry = buf[i..].to_vec();
                    return out;
                }
                i += 2;
                continue;
            };
            let body = &buf[body_start..body_end];
            let s = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
            let mark = if let Some(rest) = body.strip_prefix(b"133;") {
                match rest.first() {
                    Some(b'A') => Some((Mark::Prompt, false)),
                    Some(b'B') => Some((Mark::InputStart, false)),
                    Some(b'C') => Some((Mark::Output, false)),
                    Some(b'D') => {
                        let st = rest.get(2..).map(|r| s(r).split(';').next().unwrap_or("").trim().parse().unwrap_or(0)).unwrap_or(0);
                        Some((Mark::Done(st), false))
                    }
                    _ => None,
                }
            } else if let Some(rest) = body.strip_prefix(b"7772;") {
                Some((Mark::PromptText(s(rest)), false))
            } else if let Some(rest) = body.strip_prefix(b"7773;") {
                Some((Mark::Line(s(rest)), false))
            } else if let Some(rest) = body.strip_prefix(b"7;") {
                Some((Mark::Cwd(s(rest)), true)) // the terminal sees it too
            } else {
                None
            };
            let is133 = body.starts_with(b"133;");
            match mark {
                Some((m, keep)) => {
                    let upto = if keep { after } else { i };
                    flush(&mut out, &buf[text_start..upto]);
                    out.push(Piece::Mark(m));
                    text_start = after;
                }
                None if is133 => {
                    // 133 marks The Textsmith does not use (B, P...): dropped.
                    flush(&mut out, &buf[text_start..i]);
                    text_start = after;
                }
                None => {}
            }
            i = after;
        }
        flush(&mut out, &buf[text_start..]);
        out
    }
}

fn flush(out: &mut Vec<Piece>, b: &[u8]) {
    if b.is_empty() {
        return;
    }
    if let Some(Piece::Text(t)) = out.last_mut() {
        t.extend_from_slice(b);
    } else {
        out.push(Piece::Text(b.to_vec()));
    }
}

/// file://host/path%20x → /path x
pub fn cwd_path(url: &str) -> String {
    let rest = url.strip_prefix("file://").unwrap_or(url);
    let path = match rest.find('/') {
        Some(i) => &rest[i..],
        None => "/",
    };
    let b = path.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&path[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Piece {
        Piece::Text(s.as_bytes().to_vec())
    }

    #[test]
    fn marks_are_taken_out_in_order() {
        let mut sc = Scanner::default();
        let got = sc.push(b"ab\x1b]133;D;2\x07\x1b]7772;me@h:~\x07\x1b]7;file://h/tmp\x07\x1b]133;A\x07$ ls\r\n\x1b]133;C\x07out\r\n");
        assert_eq!(
            got,
            vec![
                t("ab"),
                Piece::Mark(Mark::Done(2)),
                Piece::Mark(Mark::PromptText("me@h:~".into())),
                t("\x1b]7;file://h/tmp\x07"),
                Piece::Mark(Mark::Cwd("file://h/tmp".into())),
                Piece::Mark(Mark::Prompt),
                t("$ ls\r\n"),
                Piece::Mark(Mark::Output),
                t("out\r\n"),
            ]
        );
    }

    #[test]
    fn a_mark_split_across_reads_is_held_back() {
        let mut sc = Scanner::default();
        assert_eq!(sc.push(b"x\x1b]77"), vec![t("x")]);
        assert_eq!(sc.push(b"73;ls -l"), vec![]);
        assert_eq!(sc.push(b"a\x07y\x1b"), vec![Piece::Mark(Mark::Line("ls -la".into())), t("y")]);
        assert_eq!(sc.push(b"[1mz"), vec![t("\x1b[1mz")]);
    }

    #[test]
    fn other_sequences_pass_through() {
        let mut sc = Scanner::default();
        assert_eq!(sc.push(b"\x1b]0;title\x07\x1b]8;;http://x\x1b\\link"), vec![t("\x1b]0;title\x07\x1b]8;;http://x\x1b\\link")]);
        assert_eq!(sc.push(b"\x1b]133;B\x07q"), vec![Piece::Mark(Mark::InputStart), t("q")]);
        assert_eq!(sc.push(b"\x1b]133;P\x07q"), vec![t("q")]);
        assert_eq!(sc.push(b"\x1b]133;D\x1b\\"), vec![Piece::Mark(Mark::Done(0))]);
    }

    #[test]
    fn cwd_url_to_path() {
        assert_eq!(cwd_path("file://forge/home/ishe/My%20Music"), "/home/ishe/My Music");
        assert_eq!(cwd_path("file:///tmp"), "/tmp");
    }
}
