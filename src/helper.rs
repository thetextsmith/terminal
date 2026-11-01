//! `textsmith bm ...`: what the shell integration calls while you edit a line.
//! It runs without GTK, so it starts fast.
//!
//!   textsmith bm expand LINE     #@ additions. exit 0: the command; 1: an error; 3: no #@
//!   textsmith bm call LINE       "name args" as a bookmark call. exit 0 or 1
//!   textsmith bm complete TEXT   TEXT is the line up to the cursor. exit 3: not a
//!                                #@ name; exit 0: line 1 is the new TEXT, the rest a message
//!   textsmith bm add LINE        Alt+M. Prints the sentence to show
//!   textsmith bm add-run LINE    Alt+Shift+M. Sentence, then "run", "edit" or "none", then the line
//!   textsmith bm list            every bookmark with its description
//!   textsmith bm export [--prefix P] [--keep-clashes] [-o FILE]
//!                                bookmarks as bash/zsh functions, for other terminals

use crate::bookmarks as bm;
use crate::store;

pub fn main(args: &[String]) -> i32 {
    let op = args.first().map(String::as_str).unwrap_or("");
    let arg = args.get(1).map(String::as_str).unwrap_or("");
    match op {
        "expand" => {
            let list = store::read();
            match bm::expand_inline(arg, |n| bm::find(&list, n)) {
                None => 3,
                Some(Ok(c)) => {
                    println!("{c}");
                    0
                }
                Some(Err(e)) => {
                    println!("{e}");
                    1
                }
            }
        }
        "call" => match bm::call(&store::read(), arg) {
            Ok(c) => {
                println!("{c}");
                0
            }
            Err(e) => {
                println!("{e}");
                1
            }
        },
        "complete" => match complete(&store::read(), arg) {
            None => 3,
            Some((text, msg)) => {
                println!("{text}");
                if !msg.is_empty() {
                    println!("{msg}");
                }
                0
            }
        },
        "add" => match store::toggle(arg, false) {
            Ok((msg, _)) => {
                println!("{msg}");
                0
            }
            Err(e) => {
                println!("{e}");
                1
            }
        },
        "add-run" => match store::toggle(arg, true) {
            Err(e) => {
                println!("{e}\nnone\n");
                1
            }
            Ok((msg, b)) => {
                if !b.name.is_empty() && b.wants_args() {
                    println!("{msg}. Type its arguments, then Enter.\nedit\n#@{} ", b.name);
                } else if b.min == 0 {
                    match bm::expand(&b, &[]) {
                        Ok(c) => println!("{msg}\nrun\n{c}"),
                        Err(e) => println!("{msg}. {e}\nnone\n"),
                    }
                } else {
                    println!("{msg}. Fill in the places marked with angle brackets, then Enter.\nedit\n{}", b.command);
                }
                0
            }
        },
        "export" => {
            let (mut prefix, mut file, mut keep) = (String::new(), None::<String>, false);
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--prefix" | "-p" => {
                        prefix = args.get(i + 1).cloned().unwrap_or_default();
                        i += 1;
                    }
                    "--keep-clashes" => keep = true,
                    "-o" | "--output" => {
                        file = args.get(i + 1).cloned().filter(|f| f != "-");
                        i += 1;
                    }
                    other => {
                        eprintln!("textsmith bm export: unknown option {other}");
                        return 2;
                    }
                }
                i += 1;
            }
            let (text, warnings) = crate::export::export(&store::read(), &prefix, keep);
            for w in &warnings {
                eprintln!("{w}");
            }
            match file {
                None => print!("{text}"),
                Some(f) => {
                    if let Err(e) = std::fs::write(&f, &text) {
                        eprintln!("Could not write {f}: {e}");
                        return 1;
                    }
                    eprintln!("Wrote {} functions to {f}.", text.matches("() {").count());
                }
            }
            0
        }
        "list" => {
            for b in store::read() {
                println!("{}    ({})", bm::format(&b), bm::describe(&b));
            }
            0
        }
        _ => {
            eprintln!("textsmith bm: expand | call | complete | add | add-run | list | export");
            2
        }
    }
}

/// Is the word before the cursor a bookmark name to complete: "#@co", or
/// "@co" after an earlier #@?
pub fn bookmark_word(before: &str) -> bool {
    let start = before.rfind(char::is_whitespace).map_or(0, |i| i + before[i..].chars().next().unwrap().len_utf8());
    let word = &before[start..];
    word.starts_with("#@") || (word.starts_with('@') && bm::inline_at(before).map_or(false, |at| at < start))
}

/// Tab on "#@co" (or "@co" after an earlier #@) completes bookmark names.
/// Takes the line up to the cursor; gives the new text and what to say.
pub fn complete(list: &[bm::Bookmark], before: &str) -> Option<(String, String)> {
    let start = before.rfind(char::is_whitespace).map_or(0, |i| i + before[i..].chars().next().unwrap().len_utf8());
    let word = &before[start..];
    let p = if word.starts_with("#@") {
        2
    } else if word.starts_with('@') && bm::inline_at(before).map_or(false, |at| at < start) {
        1
    } else {
        return None;
    };
    let typed = word[p..].to_lowercase();
    let found: Vec<&bm::Bookmark> = list.iter().filter(|b| !b.name.is_empty() && b.name.to_lowercase().starts_with(&typed)).collect();
    let head = &before[..start + p];
    match found.len() {
        0 => Some((before.to_string(), if list.iter().any(|b| !b.name.is_empty()) { format!("No bookmark starts with {}.", &word[p..]) } else { "There are no named bookmarks yet.".into() })),
        1 => Some((format!("{head}{} ", found[0].name), bm::describe(found[0]))),
        n => {
            let mut common: Vec<char> = found[0].name.chars().collect();
            for b in &found {
                let k = common.iter().zip(b.name.chars()).take_while(|(a, c)| a.eq_ignore_ascii_case(c)).count();
                common.truncate(k);
            }
            let common: String = common.into_iter().collect();
            let text = if common.chars().count() > typed.chars().count() { format!("{head}{common}") } else { before.to_string() };
            let names: Vec<&str> = found.iter().map(|b| b.name.as_str()).collect();
            Some((text, format!("{n} bookmarks: {}", names.join(", "))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_names_after_hash_at() {
        let list: Vec<_> = ["#|count: wc -l", "#|cut: cut -c1-<n:80>", "#vps: ssh forge", "ls -la"].iter().map(|l| bm::parse(l)).collect();
        assert_eq!(complete(&list, "ls #@cou"), Some(("ls #@count ".into(), "count. piped into. no arguments".into())));
        assert_eq!(complete(&list, "ls #@c"), Some(("ls #@c".into(), "2 bookmarks: count, cut".into())));
        assert_eq!(complete(&list, "#@v"), Some(("#@vps ".into(), "vps. no arguments".into())));
        assert_eq!(complete(&list, "find . #@cut 3 @co"), Some(("find . #@cut 3 @count ".into(), "count. piped into. no arguments".into())));
        assert_eq!(complete(&list, "echo @co"), None);
        assert_eq!(complete(&list, "ls -l"), None);
        assert_eq!(complete(&list, "ls #@x").unwrap().1, "No bookmark starts with x.");
    }
}
