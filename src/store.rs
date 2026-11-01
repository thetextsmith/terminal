//! The Textsmith's folder: the bookmarks file (shared with the Electron
//! version) and the GTK version's own preferences, gtk-prefs.json.

use crate::bookmarks::{self as bm, Bookmark};
use std::fs;
use std::path::PathBuf;

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// The folder, first found wins: TEXTSMITH_CONFIG (or TEXTSMITH_HOME);
/// ~/.textsmith, if it is there; else $XDG_CONFIG_HOME/textsmith, which is
/// ~/.config/textsmith on most systems.
pub fn config_dir() -> PathBuf {
    for v in ["TEXTSMITH_CONFIG", "TEXTSMITH_HOME"] {
        if let Some(p) = std::env::var_os(v).filter(|p| !p.is_empty()) {
            return PathBuf::from(p);
        }
    }
    let old = home().join(".textsmith");
    if old.is_dir() {
        return old;
    }
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(".config"));
    xdg.join("textsmith")
}

pub fn bookmarks_file() -> PathBuf {
    config_dir().join("bookmarks")
}

const HEADER: &str = "# The Textsmith bookmarks: one per line, written as you would type them.
#   #name(count) description: command      or just: command
# <> or <name> marks where an argument goes; <name:default> fills in default when left out;
# <..> takes any number of arguments.
# Lines starting with \"# \" are notes.
";

/// The bookmark lines, notes left out. Notes start with "# " or "##".
pub fn read_lines() -> Vec<String> {
    let text = fs::read_to_string(bookmarks_file()).unwrap_or_default();
    text.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !(l == "#" || l.starts_with("# ") || l.starts_with("#\t") || l.starts_with("##")) && !l.starts_with("machine:")) // an old way of naming the machine: ignored
        .collect()
}

pub fn read() -> Vec<Bookmark> {
    read_lines().iter().map(|l| bm::parse(l)).collect()
}

/// Writes the list back. Notes the user wrote in the file are kept at the top.
pub fn write_lines(list: &[String]) -> std::io::Result<()> {
    fs::create_dir_all(config_dir())?;
    let old = fs::read_to_string(bookmarks_file()).unwrap_or_default();
    let notes: Vec<&str> = old
        .lines()
        .filter(|l| {
            let l = l.trim();
            l == "#" || l.starts_with("# ") || l.starts_with("##")
        })
        .collect();
    let mut out = if notes.is_empty() { HEADER.to_string() } else { notes.join("\n") + "\n" };
    for l in list {
        out.push_str(&l.replace('\n', " "));
        out.push('\n');
    }
    let tmp = bookmarks_file().with_extension("tmp");
    fs::write(&tmp, out)?;
    fs::rename(tmp, bookmarks_file())
}

/// What Alt+M does with a line: bookmark it, or remove it when the same
/// bookmark is already there. The same name with a new command replaces the
/// old one. Returns the sentence to show and the bookmark.
pub fn toggle(line: &str, keep_existing: bool) -> Result<(String, Bookmark), String> {
    let typed = line.trim();
    if typed.is_empty() {
        return Err("Type a command to bookmark.".into());
    }
    let b = bm::parse(typed);
    let text = bm::format(&b);
    let mut list = read_lines();
    if let Some(i) = list.iter().position(|r| bm::format(&bm::parse(r)) == text) {
        if !keep_existing {
            list.remove(i);
            write_lines(&list).map_err(|e| format!("Could not save bookmarks: {e}"))?;
            return Ok((format!("Bookmark removed: {}", if b.name.is_empty() { &b.command } else { &b.name }), b));
        }
        return Ok((format!("Already bookmarked: {}", what(&b)), b));
    }
    let old = list.iter().position(|r| {
        let o = bm::parse(r);
        if b.name.is_empty() {
            o.name.is_empty() && o.command == b.command
        } else {
            o.name == b.name
        }
    });
    let updated = old.is_some();
    if let Some(i) = old {
        list.remove(i);
    }
    list.push(text);
    write_lines(&list).map_err(|e| format!("Could not save bookmarks: {e}"))?;
    Ok((format!("{}: {}", if updated { "Bookmark updated" } else { "Bookmarked" }, what(&b)), b))
}

fn what(b: &Bookmark) -> String {
    if b.name.is_empty() {
        b.command.clone()
    } else {
        bm::describe(b)
    }
}

pub fn remove(raw: &str) -> std::io::Result<()> {
    let mut list = read_lines();
    list.retain(|r| r != raw);
    write_lines(&list)
}
