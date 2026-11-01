//! One tab: a terminal, the shell on its pty, and the review buffer (F4).
//!
//! What the shell writes goes through the scanner. Each mark it finds is
//! replaced by a harmless query (XTVERSION); the terminal answers it at the
//! very point in the stream where the mark was, so the cursor position then
//! is exactly where the command's output started or ended.

use crate::config::Prefs;
use crate::pty::Pty;
use crate::review::{self, Block, Heading};
use crate::scan::{self, Mark, Piece, Scanner};
use crate::vte;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

const BARRIER: &[u8] = b"\x1b[>0q";
const MAX_BLOCKS: usize = 2000;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Starting,
    Prompt,
    Running,
}

pub struct Inner {
    pub terminal: vte::Terminal,
    pub stack: gtk::Stack,
    pub review: gtk::TextView,
    /// The review's colours when the preferences choose them (not "system").
    pub review_colours: gtk::CssProvider,
    pub pty: Pty,
    pub shell: String,
    pub tmp: PathBuf,
    scanner: RefCell<Scanner>,
    marks: RefCell<VecDeque<Mark>>,
    pub state: Cell<State>,
    lines: RefCell<Vec<String>>,
    input_start: Cell<Option<(i64, i64)>>, // where typing starts at this prompt
    from_screen: Cell<bool>,               // `lines` was read from the screen
    pub blocks: RefCell<Vec<Block>>,
    current: Cell<Option<usize>>,
    pub prompt: RefCell<String>,
    pub cwd: RefCell<String>,
    launch: RefCell<Option<String>>,
    heads: RefCell<Vec<Heading>>,
    pub prefs: RefCell<Prefs>,
    on_change: RefCell<Option<Box<dyn Fn()>>>,
    on_exit: RefCell<Option<Box<dyn Fn()>>>,
    last_size: Cell<(u16, u16)>,
}

#[derive(Clone)]
pub struct Tab(pub Rc<Inner>);

pub struct Start {
    pub shell: String,
    pub integration: PathBuf,
    pub cwd: PathBuf,
    pub command: Option<String>,
    pub tmp: PathBuf,
}

impl Tab {
    pub fn new(prefs: &Prefs, start: Start) -> std::io::Result<Tab> {
        let terminal = vte::Terminal::new();
        terminal.set_scrollback(prefs.n("display", "scrollback").max(1000));
        let review = gtk::TextView::new();
        review.set_editable(false);
        review.set_cursor_visible(true);
        review.set_monospace(true);
        review.set_wrap_mode(gtk::WrapMode::WordChar);
        review.set_left_margin(6);
        if let Some(a) = review.accessible() {
            a.set_name("Review");
        }
        let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scroll.add(&review);
        let stack = gtk::Stack::new();
        // A little room around the terminal, in the terminal's own background.
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        frame.style_context().add_class("ts-frame");
        terminal.widget.set_margin_start(10);
        terminal.widget.set_margin_end(10);
        terminal.widget.set_margin_top(6);
        terminal.widget.set_margin_bottom(4);
        frame.pack_start(&terminal.widget, true, true, 0);
        stack.add_named(&frame, "terminal");
        stack.add_named(&scroll, "review");
        terminal.widget.set_hexpand(true);
        terminal.widget.set_vexpand(true);

        let kind = Path::new(&start.shell).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let mut env: Vec<(String, OsString)> = vec![
            ("TERM".into(), "xterm-256color".into()),
            ("COLORTERM".into(), "truecolor".into()),
            ("VTE_VERSION".into(), "7600".into()),
            ("TEXTSMITH".into(), "1".into()),
            ("TEXTSMITH_GTK".into(), env!("CARGO_PKG_VERSION").into()),
            ("TEXTSMITH_TMP".into(), start.tmp.clone().into()),
            ("SHELL".into(), start.shell.clone().into()),
            ("TEXTSMITH_TAB".into(), prefs.s("editing", "tabCompletion").into()),
        ];
        if let Ok(exe) = std::env::current_exe() {
            env.push(("TEXTSMITH_BIN".into(), exe.into()));
        }
        let argv: Vec<OsString> = if kind == "zsh" {
            env.push(("ZDOTDIR".into(), start.integration.join("zsh").into()));
            env.push(("TEXTSMITH_USER_ZDOTDIR".into(), std::env::var_os("ZDOTDIR").unwrap_or_default()));
            vec![start.shell.clone().into(), "-i".into()]
        } else {
            vec![start.shell.clone().into(), "--rcfile".into(), start.integration.join("textsmith.bashrc").into(), "-i".into()]
        };
        let pty = Pty::spawn(&argv, &env, &start.cwd, 80, 24)?;
        let inner = Rc::new(Inner {
            terminal,
            stack,
            review: review.clone(),
            review_colours: {
                let c = gtk::CssProvider::new();
                review.style_context().add_provider(&c, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
                c
            },
            pty,
            shell: kind,
            tmp: start.tmp,
            scanner: RefCell::default(),
            marks: RefCell::default(),
            state: Cell::new(State::Starting),
            lines: RefCell::default(),
            input_start: Cell::new(None),
            from_screen: Cell::new(false),
            blocks: RefCell::default(),
            current: Cell::new(None),
            prompt: RefCell::default(),
            cwd: RefCell::new(start.cwd.to_string_lossy().into_owned()),
            launch: RefCell::new(start.command),
            heads: RefCell::default(),
            prefs: RefCell::new(prefs.clone()),
            on_change: RefCell::default(),
            on_exit: RefCell::default(),
            last_size: Cell::new((0, 0)),
        });
        let tab = Tab(inner);
        tab.apply_prefs(prefs);
        tab.wire();
        Ok(tab)
    }

    fn weak(&self) -> Weak<Inner> {
        Rc::downgrade(&self.0)
    }

    fn wire(&self) {
        let t = &self.0;
        // Shell output → scanner → terminal.
        let w = self.weak();
        glib::source::unix_fd_add_local(t.pty.fd(), glib::IOCondition::IN | glib::IOCondition::HUP | glib::IOCondition::ERR, move |_, _| {
            let Some(t) = w.upgrade() else { return glib::ControlFlow::Break };
            let mut buf = vec![0u8; 65536];
            for _ in 0..16 {
                match t.pty.read(&mut buf) {
                    Some(0) => return glib::ControlFlow::Continue,
                    Some(n) => Tab(t.clone()).incoming(&buf[..n]),
                    None => return glib::ControlFlow::Break,
                }
            }
            glib::ControlFlow::Continue
        });
        // Keys and replies → shell; our own query's answer → the next mark.
        let w = self.weak();
        t.terminal.connect_commit(move |bytes| {
            let Some(t) = w.upgrade() else { return };
            if bytes.starts_with(b"\x1bP>|") {
                let m = t.marks.borrow_mut().pop_front();
                if let Some(m) = m {
                    let pos = t.terminal.cursor();
                    Tab(t.clone()).mark(m, pos);
                    return;
                }
            }
            t.pty.write(bytes);
        });
        // The shell ends: the tab closes.
        let w = self.weak();
        glib::child_watch_add_local(glib::Pid(t.pty.pid), move |_, _| {
            if let Some(t) = w.upgrade() {
                if let Some(f) = t.on_exit.borrow().as_ref() {
                    f()
                }
            }
        });
        // The terminal's size → the pty's.
        let w = self.weak();
        t.terminal.widget.connect_size_allocate(move |_, _| {
            if let Some(t) = w.upgrade() {
                Tab(t).sync_size();
            }
        });
    }

    pub fn sync_size(&self) {
        let t = &self.0;
        let size = t.terminal.size();
        if size != t.last_size.get() && size.0 > 1 && size.1 > 1 {
            t.last_size.set(size);
            t.pty.resize(size.0, size.1);
        }
    }

    fn incoming(&self, data: &[u8]) {
        let t = &self.0;
        let pieces = t.scanner.borrow_mut().push(data);
        for p in pieces {
            match p {
                Piece::Text(b) => t.terminal.feed(&b),
                Piece::Mark(m) => {
                    t.marks.borrow_mut().push_back(m);
                    t.terminal.feed(BARRIER);
                }
            }
        }
    }

    fn mark(&self, m: Mark, pos: (i64, i64)) {
        let t = &self.0;
        match m {
            Mark::Prompt => {
                t.state.set(State::Prompt);
                t.lines.borrow_mut().clear();
                t.input_start.set(None);
                t.from_screen.set(false);
                if let Some(c) = t.launch.borrow_mut().take() {
                    self.put_line(&c, true);
                }
            }
            Mark::InputStart => {
                if t.state.get() == State::Prompt {
                    t.input_start.set(Some(pos));
                }
            }
            Mark::Line(l) => {
                if t.state.get() == State::Prompt {
                    // What the shell says ran wins over what was read from the screen.
                    if t.from_screen.replace(false) {
                        t.lines.borrow_mut().clear();
                    }
                    t.lines.borrow_mut().push(l);
                }
            }
            Mark::Output => {
                if t.state.get() != State::Running {
                    // A line read from the screen at Enter may have missed the last
                    // letters' echo; now that the output starts, all of it is there.
                    if t.from_screen.get() {
                        if let Some(start) = t.input_start.get() {
                            let line = t.terminal.text(start, pos).trim_end().replace("\n> ", "\n");
                            if !line.trim().is_empty() {
                                *t.lines.borrow_mut() = vec![line];
                            }
                        }
                    }
                    let lines = std::mem::take(&mut *t.lines.borrow_mut());
                    let mut blocks = t.blocks.borrow_mut();
                    if blocks.len() >= MAX_BLOCKS {
                        blocks.drain(..MAX_BLOCKS / 10);
                    }
                    blocks.push(Block {
                        command: if lines.is_empty() { "(a command)".into() } else { lines.join("\n") },
                        dir: tilde(&t.cwd.borrow()),
                        prompt: t.prompt.borrow().clone(),
                        start: pos,
                        status: None,
                        text: String::new(),
                    });
                    t.current.set(Some(blocks.len() - 1));
                    t.state.set(State::Running);
                }
            }
            Mark::Done(st) => {
                if let Some(i) = t.current.take() {
                    let mut blocks = t.blocks.borrow_mut();
                    if let Some(b) = blocks.get_mut(i) {
                        b.status = Some(st);
                        b.text = output(&t.terminal, b.start, pos);
                    }
                }
            }
            Mark::Cwd(url) => *t.cwd.borrow_mut() = scan::cwd_path(&url),
            Mark::PromptText(p) => *t.prompt.borrow_mut() = p,
        }
        if let Some(f) = t.on_change.borrow().as_ref() {
            f()
        }
    }

    pub fn at_prompt(&self) -> bool {
        self.0.state.get() == State::Prompt
    }

    /// Put a line on the shell's command line, and maybe run it.
    pub fn put_line(&self, line: &str, run: bool) {
        let t = &self.0;
        if std::fs::write(t.tmp.join("line"), line).is_ok() {
            t.pty.write(if run { b"\x1b[9986~" } else { b"\x1b[9985~" });
        }
    }

    /// Add to the end of what is on the shell's command line.
    pub fn append_line(&self, text: &str) {
        let t = &self.0;
        if std::fs::write(t.tmp.join("line"), text).is_ok() {
            t.pty.write(b"\x1b[9988~");
        }
    }

    /// Enter at the prompt. The line is read from the screen; a line with
    /// #@ goes through the shell's hook to be expanded, any other line is
    /// left to the shell untouched (true: the key is handled here).
    pub fn enter(&self) -> bool {
        let t = &self.0;
        let Some(start) = t.input_start.get() else {
            self.send(b"\x1b[9989~");
            return true;
        };
        let cur = t.terminal.cursor();
        let line = t.terminal.text(start, (0, cur.1 + 1)).trim_end().to_string();
        if crate::bookmarks::inline_at(line.trim_start()).is_some() {
            self.send(b"\x1b[9989~");
            return true;
        }
        // A command over several lines: the text since the prompt, without the "> " of PS2.
        let line = line.replace("\n> ", "\n");
        *t.lines.borrow_mut() = vec![line];
        t.from_screen.set(true);
        false
    }

    /// Tab at the prompt: a #@ name is completed by the shell's hook; any
    /// other word is left to the shell's own completion (true: handled here).
    pub fn tab(&self, back: bool) -> bool {
        let t = &self.0;
        let Some(start) = t.input_start.get() else { return false };
        let before = t.terminal.text(start, t.terminal.cursor());
        let before = before.trim_end_matches('\n');
        if !back && crate::helper::bookmark_word(before) {
            self.send(b"\x1b[9978~");
            return true;
        }
        // Cycling through choices replaces part of the word, and Orca would
        // hear only that part ("6.0"): the whole choice is said instead.
        let word = |s: &str| s.trim_end().rsplit(char::is_whitespace).next().unwrap_or("").to_string();
        let was = word(before);
        let w = self.weak();
        glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
            let Some(t) = w.upgrade() else { return };
            if t.state.get() != State::Prompt || t.input_start.get() != Some(start) {
                return;
            }
            let now = word(t.terminal.text(start, t.terminal.cursor()).trim_end_matches('\n'));
            if now != was && (back || !now.starts_with(&was)) && !now.is_empty() {
                crate::app::announce_now(&t.terminal.widget, &now);
            }
        });
        false
    }

    pub fn send(&self, b: &[u8]) {
        self.0.pty.write(b);
    }

    pub fn connect_change(&self, f: impl Fn() + 'static) {
        *self.0.on_change.borrow_mut() = Some(Box::new(f));
    }
    pub fn connect_exit(&self, f: impl Fn() + 'static) {
        *self.0.on_exit.borrow_mut() = Some(Box::new(f));
    }

    /// The names a running ssh's machine may go by, best first: the host as
    /// given to ssh (an alias from ~/.ssh/config, say), then its real host
    /// name, in full and short. None when the program is not ssh or mosh.
    pub fn remote_names(&self) -> Option<Vec<String>> {
        let argv = self.0.pty.foreground_argv().filter(|a| crate::app::remote_machine_argv(a).is_some());
        let host = match argv {
            Some(a) => crate::app::remote_machine_argv(&a)?,
            None => crate::app::remote_machine(&self.running_command()?)?,
        };
        let mut names = vec![host.clone()];
        if let Ok(out) = std::process::Command::new("ssh").args(["-G", &host]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(h) = text.lines().find_map(|l| l.strip_prefix("hostname ")) {
                for n in [h.trim().to_string(), h.trim().split('.').next().unwrap_or("").to_string()] {
                    if !n.is_empty() && !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
        }
        if let Some(short) = host.split('.').next().filter(|s| *s != host) {
            if !names.iter().any(|n| n == short) {
                names.push(short.to_string());
            }
        }
        Some(names)
    }

    pub fn running_command(&self) -> Option<String> {
        let i = self.0.current.get()?;
        self.0.blocks.borrow().get(i).map(|b| b.command.lines().next().unwrap_or("").to_string())
    }

    /// The window title for this tab, from the format in the preferences.
    pub fn title(&self) -> String {
        let t = &self.0;
        let prefs = t.prefs.borrow();
        let running = t.state.get() == State::Running;
        let fmt = if running { prefs.s("title", "running") } else { prefs.s("title", "format") };
        let mut prompt = t.prompt.borrow().trim().to_string();
        while let Some(c) = prompt.chars().last() {
            if "$#%>❯".contains(c) || c.is_whitespace() {
                prompt.pop();
            } else {
                break;
            }
        }
        if prompt.is_empty() {
            prompt = format!("{}@{}: {}", user(), host(), tilde(&t.cwd.borrow()));
        }
        let command = self.running_command().unwrap_or_default();
        let s = fmt
            .replace("{prompt}", &prompt)
            .replace("{dir}", &tilde(&t.cwd.borrow()))
            .replace("{shell}", &t.shell)
            .replace("{command}", &command)
            .replace("{app}", "The Textsmith");
        s.trim_matches(|c: char| c == ' ' || c == '—').to_string()
    }

    /// A short name for the tab: the directory's last part.
    pub fn short_title(&self) -> String {
        let d = tilde(&self.0.cwd.borrow());
        if d == "~" || d == "/" {
            return d;
        }
        d.rsplit('/').next().unwrap_or(&d).to_string()
    }

    pub fn apply_prefs(&self, prefs: &Prefs) {
        let t = &self.0;
        *t.prefs.borrow_mut() = prefs.clone();
        let font = prefs.s("display", "font");
        if !font.is_empty() {
            let fd = pango::FontDescription::from_string(&font);
            t.terminal.set_font(&fd);
            // The review buffer in the same font, through a style sheet.
            let family = fd.family().map(|f| f.to_string()).unwrap_or_else(|| "Monospace".into());
            let size = (fd.size() / pango::SCALE).max(6);
            let css = format!("textview, textview text {{ font-family: \"{}\"; font-size: {}pt; }}", family.replace('"', ""), size);
            let provider = gtk::CssProvider::new();
            if provider.load_from_data(css.as_bytes()).is_ok() {
                t.review.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
            }
        }
        t.terminal.set_audible_bell(prefs.b("display", "bell"));
        let colours = prefs.s("display", "colours");
        let pair = |fg: &str, bg: &str| (fg.parse::<gdk::RGBA>().unwrap(), bg.parse::<gdk::RGBA>().unwrap());
        let chosen = match colours.as_str() {
            "dark" => Some(pair("#d8d8d8", "#1c1c1c")),
            "light" => Some(pair("#1c1c1c", "#ffffff")),
            "high-contrast" => Some(pair("#ffffff", "#000000")),
            _ => {
                // The desktop's own text colours, so a light or dark theme carries over.
                let ctx = t.review.style_context();
                match (ctx.lookup_color("theme_text_color"), ctx.lookup_color("theme_base_color")) {
                    (Some(f), Some(b)) => Some((f, b)),
                    _ => None,
                }
            }
        };
        let (fg, bg) = chosen.unwrap_or_else(|| pair("#171421", "#ffffff"));
        // The review buffer takes chosen colours too; with "system" it keeps the theme's.
        let css = if colours == "system" || colours.is_empty() {
            String::new()
        } else {
            format!("textview text {{ color: {f}; background-color: {b}; caret-color: {f}; }}", f = fg.to_str(), b = bg.to_str())
        };
        let _ = t.review_colours.load_from_data(css.as_bytes());
        // GNOME Terminal's palette on a light background; brighter colours on
        // a dark one, so blue directory names stay readable.
        let dark = 0.2126 * bg.red() + 0.7152 * bg.green() + 0.0722 * bg.blue() < 0.5;
        let colours: [&str; 16] = if dark {
            [
                "#3d3846", "#f66151", "#33d17a", "#e9ad0c", "#62a0ea", "#c061cb", "#33c7de", "#d0cfcc",
                "#77767b", "#ff7b63", "#57e389", "#f8e45c", "#99c1f1", "#dc8add", "#93ddc2", "#ffffff",
            ]
        } else {
            [
                "#171421", "#c01c28", "#26a269", "#a2734c", "#12488b", "#a347ba", "#2aa1b3", "#d0cfcc",
                "#5e5c64", "#f66151", "#33d17a", "#e9ad0c", "#2a7bde", "#c061cb", "#33c7de", "#ffffff",
            ]
        };
        let palette: Vec<gdk::RGBA> = colours.iter().map(|c| c.parse().unwrap()).collect();
        t.terminal.set_colors(&fg, &bg, &palette);
        if let Some(frame) = t.terminal.widget.parent() {
            let provider = gtk::CssProvider::new();
            let css = format!(".ts-frame {{ background-color: {}; }}", bg.to_str());
            if provider.load_from_data(css.as_bytes()).is_ok() {
                frame.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
            }
        }
    }

    // ---------------------------------------------------------------- review

    pub fn review_open(&self) -> bool {
        self.0.stack.visible_child_name().as_deref() == Some("review")
    }

    /// F4: the review buffer, with the cursor on the last command.
    pub fn open_review(&self) {
        let t = &self.0;
        self.refresh_review();
        t.stack.set_visible_child_name("review");
        t.review.grab_focus();
        let last = t.heads.borrow().last().map(|h| h.line);
        match last {
            Some(l) => self.goto_line(l, false),
            None => crate::app::announce(&t.review.clone().upcast(), "Review. No commands yet."),
        }
    }

    pub fn refresh_review(&self) {
        let t = &self.0;
        let fmt = t.prefs.borrow().s("review", "command");
        let term = t.terminal.clone();
        let (text, heads) = review::build(&t.blocks.borrow(), &fmt, |b| output(&term, b.start, term.cursor()));
        let buf = t.review.buffer().unwrap();
        buf.set_text(&text);
        // For sighted readers: each command line bold, a failed one in red.
        // Only the look changes; the text Orca reads is the same.
        let table = buf.tag_table().unwrap();
        let tag = |name: &str, colour: &str| {
            table.lookup(name).unwrap_or_else(|| {
                let t = gtk::TextTag::new(Some(name));
                t.set_weight(700);
                t.set_foreground(Some(colour));
                t.set_pixels_above_lines(8);
                table.add(&t);
                t
            })
        };
        let (ok_tag, bad_tag) = (tag("command", "#3584e4"), tag("failed", "#e01b24"));
        for h in &heads {
            let a = buf.iter_at_line(h.line);
            let b = buf.iter_at_line(h.line + h.lines);
            buf.apply_tag(if h.failed { &bad_tag } else { &ok_tag }, &a, &b);
        }
        *t.heads.borrow_mut() = heads;
    }

    pub fn close_review(&self) {
        let t = &self.0;
        t.stack.set_visible_child_name("terminal");
        t.terminal.widget.grab_focus();
    }

    fn goto_line(&self, line: i32, speak: bool) {
        let t = &self.0;
        let buf = t.review.buffer().unwrap();
        let mut it = buf.iter_at_line(line);
        buf.place_cursor(&it);
        t.review.scroll_to_iter(&mut it, 0.1, true, 0.0, 0.0);
        if speak || t.prefs.borrow().b("review", "speakJumps") {
            let mut end = it.clone();
            end.forward_to_line_end();
            let text = buf.text(&it, &end, false).map(|s| s.to_string()).unwrap_or_default();
            crate::app::announce(&t.review.clone().upcast(), &text);
        }
    }

    /// Alt+Up / Alt+Down and the failed-only variants.
    pub fn jump(&self, forward: bool, failed_only: bool) {
        let t = &self.0;
        let buf = t.review.buffer().unwrap();
        let here = buf.iter_at_mark(&buf.get_insert().unwrap()).line();
        let heads = t.heads.borrow().clone();
        let pick = if forward {
            heads.iter().find(|h| h.line > here && (!failed_only || h.failed))
        } else {
            heads.iter().rev().find(|h| h.line < here && (!failed_only || h.failed))
        };
        match pick {
            Some(h) => self.goto_line(h.line, false),
            None => {
                let what = match (forward, failed_only) {
                    (true, false) => "No later command.",
                    (false, false) => "No earlier command.",
                    (true, true) => "No later command failed.",
                    (false, true) => "No earlier command failed.",
                };
                crate::app::announce(&t.review.clone().upcast(), what);
            }
        }
    }

    pub fn jump_last(&self) {
        let last = self.0.heads.borrow().last().map(|h| h.line);
        if let Some(l) = last {
            self.goto_line(l, false);
        }
    }

    pub fn close(&self) {
        self.0.pty.hangup();
        let _ = std::fs::remove_dir_all(&self.0.tmp);
    }
}

/// A command's output from the terminal. Lines that have gone past the
/// scrollback are said to be gone rather than shown as blank lines.
fn output(term: &vte::Terminal, start: (i64, i64), end: (i64, i64)) -> String {
    // The terminal keeps about `scrollback` lines; rows before that come back blank.
    let first = end.1 - term.scrollback() + 1;
    if start.1 >= first {
        return review::tidy(&term.text(start, end));
    }
    let text = review::tidy(&term.text((0, first), end));
    let lines: Vec<&str> = text.lines().collect();
    let blank = lines.iter().take_while(|l| l.is_empty()).count();
    let gone = first - start.1 + blank as i64;
    format!(
        "({gone} earlier lines are past the scrollback; display.scrollback in the preferences keeps more)\n{}",
        lines[blank..].join("\n")
    )
}

pub fn user() -> String {
    std::env::var("USER").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        let pw = unsafe { libc::getpwuid(libc::getuid()) };
        if pw.is_null() {
            return "user".into();
        }
        unsafe { std::ffi::CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned() }
    })
}

pub fn host() -> String {
    let mut buf = [0u8; 256];
    unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    let s = String::from_utf8_lossy(&buf).trim_end_matches('\0').to_string();
    s.split('.').next().unwrap_or(&s).to_string()
}

pub fn tilde(p: &str) -> String {
    let home = crate::store::home().to_string_lossy().into_owned();
    if !home.is_empty() && home != "/" {
        if p == home {
            return "~".into();
        }
        if let Some(rest) = p.strip_prefix(&(home + "/")) {
            return format!("~/{rest}");
        }
    }
    p.to_string()
}
