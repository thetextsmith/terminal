//! The window: tabs, keys, the bookmark list, preferences.

use crate::bookmarks as bm;
use crate::config::{self, Prefs};
use crate::session::{Start, Tab};
use crate::store;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;

pub struct Launch {
    pub cwd: Option<PathBuf>,
    pub command: Option<String>,
}

struct App {
    window: gtk::Window,
    notebook: gtk::Notebook,
    status: gtk::Label,
    tabs: RefCell<Vec<Tab>>,
    prefs: RefCell<Prefs>,
    prefs_time: Cell<Option<SystemTime>>,
    integration: PathBuf,
    runtime: PathBuf,
    next_id: Cell<u32>,
}

/// Something for Orca to say now, without moving the focus. GTK 3 has no
/// call for it; ATK's "notification" signal (ATK 2.50) or "announcement"
/// (2.46) is emitted on the widget's accessible, and the AT-SPI bridge passes
/// it on as object:announcement. The status line shows it too.
pub fn announce(widget: &gtk::Widget, text: &str) {
    say(widget, text, 1); // ATK_LIVE_POLITE: after what Orca is saying
}

/// Said at once, cutting off what Orca was saying (ATK_LIVE_ASSERTIVE).
pub fn announce_now(widget: &gtk::Widget, text: &str) {
    say(widget, text, 2);
}

fn say(widget: &gtk::Widget, text: &str, politeness: i32) {
    if let Some(acc) = widget.accessible() {
        let ty = acc.type_();
        let text = text.replace('\0', "");
        if glib::subclass::signal::SignalId::lookup("notification", ty).is_some() {
            acc.emit_by_name::<()>("notification", &[&text, &politeness]);
        } else if glib::subclass::signal::SignalId::lookup("announcement", ty).is_some() {
            acc.emit_by_name::<()>("announcement", &[&text]);
        }
    }
    STATUS.with(|s| {
        if let Some(l) = s.borrow().as_ref() {
            l.set_text(text);
        }
    });
    REVEALER.with(|r| {
        if let Some(r) = r.borrow().as_ref() {
            r.set_reveal_child(true);
            HIDE.with(|h| {
                if let Some(id) = h.borrow_mut().take() {
                    id.remove();
                }
                let r = r.clone();
                let id = glib::timeout_add_local_once(std::time::Duration::from_secs(5), move || {
                    r.set_reveal_child(false);
                    HIDE.with(|h| h.borrow_mut().take());
                });
                *h.borrow_mut() = Some(id);
            });
        }
    });
    if std::env::var_os("TEXTSMITH_TEST_DUMP").is_some() {
        glib::idle_add_local_once(|| DUMP.with(|d| d.borrow().as_ref().map(|a| a.dump()).unwrap_or(())));
    }
}

thread_local! {
    static STATUS: RefCell<Option<gtk::Label>> = RefCell::new(None);
    static DUMP: RefCell<Option<Rc<App>>> = RefCell::new(None);
    static REVEALER: RefCell<Option<gtk::Revealer>> = RefCell::new(None);
    static HIDE: RefCell<Option<glib::SourceId>> = RefCell::new(None);
}

/// A little room around the text, a quiet status line and tabs.
fn style() {
    let css = b"
        textview text { }
        textview { padding: 6px 10px; }
        .ts-status { padding: 4px 12px; border: 1px solid alpha(currentColor, 0.2); border-radius: 6px; font-size: smaller; background-color: @theme_bg_color; }
        notebook > header.top > tabs > tab { padding: 4px 14px; min-height: 0; }
    ";
    let provider = gtk::CssProvider::new();
    if provider.load_from_data(css).is_ok() {
        if let Some(screen) = gdk::Screen::default() {
            gtk::StyleContext::add_provider_for_screen(&screen, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    }
}

/// The manual (doc/manual.md, made into one page by scripts/manual.sh).
const MANUAL: &str = include_str!("../doc/manual.html");
const MANUAL_URL: &str = "https://thetextsmith.com/docs/";
pub const ABOUT: &str = "The Textsmith is a powerful Linux terminal boosting productivity for anyone who works with text, commands and the shell.";
pub const COPYRIGHT: &str = "Copyright 2026 Ishe Chinyoka\nThe Textsmith is distributed under the GPL 3 license.";

pub fn run(integration: PathBuf, runtime: PathBuf, launch: Launch) {
    gtk::init().expect("GTK could not start: is there a display?");
    glib::set_application_name("The Textsmith");
    glib::set_prgname(Some("textsmith"));
    let prefs = Prefs::load();
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("The Textsmith");
    window.set_default_size(960, 680);
    window.set_icon_name(Some("utilities-terminal"));
    let notebook = gtk::Notebook::new();
    notebook.set_show_border(false);
    notebook.set_scrollable(true);
    notebook.set_can_focus(false);
    let status = gtk::Label::new(None);
    status.set_xalign(0.0);
    status.set_ellipsize(pango::EllipsizeMode::End);
    status.style_context().add_class("ts-status");
    // The status line shows a message for a few seconds, then gets out of the way.
    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::Crossfade);
    revealer.add(&status);
    STATUS.with(|s| *s.borrow_mut() = Some(status.clone()));
    REVEALER.with(|r| *r.borrow_mut() = Some(revealer.clone()));
    // The status line floats over the bottom edge: if it took room of its own,
    // the terminal would shrink each time it shows, and a terminal that changes
    // size reports all its text to Orca again.
    revealer.set_halign(gtk::Align::End);
    revealer.set_valign(gtk::Align::Start);
    revealer.set_margin_top(6);
    revealer.set_margin_end(14);
    let vbox = gtk::Overlay::new();
    vbox.add(&notebook);
    vbox.add_overlay(&revealer);
    vbox.set_overlay_pass_through(&revealer, true);
    style();
    window.add(&vbox);

    let app = Rc::new(App {
        window: window.clone(),
        notebook: notebook.clone(),
        status,
        tabs: RefCell::default(),
        prefs: RefCell::new(prefs),
        prefs_time: Cell::new(mtime()),
        integration,
        runtime,
        next_id: Cell::new(1),
    });

    if std::env::var_os("TEXTSMITH_TEST_DUMP").is_some() {
        DUMP.with(|d| *d.borrow_mut() = Some(app.clone()));
    }
    let a = app.clone();
    window.connect_key_press_event(move |_, ev| {
        let r = a.key(ev);
        let a2 = a.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(30), move || a2.dump());
        r
    });
    let a = app.clone();
    window.connect_focus_in_event(move |_, _| {
        a.reload_prefs();
        glib::Propagation::Proceed
    });
    let a = app.clone();
    notebook.connect_switch_page(move |_, _, n| {
        let a = a.clone();
        glib::idle_add_local_once(move || a.shown(n as usize));
    });
    let a = app.clone();
    window.connect_delete_event(move |_, _| {
        for t in a.tabs.borrow().iter() {
            t.close();
        }
        gtk::main_quit();
        glib::Propagation::Proceed
    });

    window.show_all();
    app.new_tab(launch.cwd, launch.command);
    gtk::main();
}

fn mtime() -> Option<SystemTime> {
    std::fs::metadata(config::file()).and_then(|m| m.modified()).ok()
}

impl App {
    fn prefs(&self) -> Prefs {
        self.prefs.borrow().clone()
    }

    fn current(&self) -> Option<Tab> {
        let i = self.notebook.current_page()? as usize;
        self.tabs.borrow().get(i).cloned()
    }

    fn shell_path(&self) -> String {
        let want = std::env::var("TEXTSMITH_SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| self.prefs().s("startup", "shell"));
        let sh = if want.is_empty() { login_shell() } else { want };
        let base = std::path::Path::new(&sh).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        if base == "bash" || base == "zsh" {
            sh
        } else {
            "bash".into()
        }
    }

    fn new_tab(self: &Rc<Self>, cwd: Option<PathBuf>, command: Option<String>) {
        let home = store::home();
        let dir = cwd.or_else(|| {
            let d = self.prefs().s("startup", "directory");
            if d.is_empty() {
                None
            } else {
                Some(PathBuf::from(d.replacen('~', &home.to_string_lossy(), 1)))
            }
        });
        let dir = dir.filter(|d| d.is_dir()).unwrap_or(home);
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let tmp = self.runtime.join(format!("tab-{}-{id}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let start = Start { shell: self.shell_path(), integration: self.integration.clone(), cwd: dir, command, tmp };
        let tab = match Tab::new(&self.prefs(), start) {
            Ok(t) => t,
            Err(e) => {
                announce(self.window.upcast_ref(), &format!("The shell could not start: {e}"));
                return;
            }
        };
        let label = gtk::Label::new(Some(&tab.short_title()));
        let n = self.notebook.append_page(&tab.0.stack, Some(&label));
        self.tabs.borrow_mut().push(tab.clone());
        let a = self.clone();
        let t = tab.clone();
        tab.connect_change(move || a.changed(&t));
        let a = self.clone();
        let t = tab.clone();
        tab.connect_exit(move || a.close_tab(&t));
        tab.0.stack.show_all();
        self.notebook.set_show_tabs(self.tabs.borrow().len() > 1);
        self.notebook.set_current_page(Some(n));
        tab.0.terminal.widget.grab_focus();
        self.changed(&tab);
    }

    fn close_tab(self: &Rc<Self>, tab: &Tab) {
        let i = self.tabs.borrow().iter().position(|t| Rc::ptr_eq(&t.0, &tab.0));
        let Some(i) = i else { return };
        tab.close();
        self.tabs.borrow_mut().remove(i);
        self.notebook.remove_page(Some(i as u32));
        if self.tabs.borrow().is_empty() {
            gtk::main_quit();
            return;
        }
        self.notebook.set_show_tabs(self.tabs.borrow().len() > 1);
    }

    /// A tab came to the front: focus it and say which it is.
    fn shown(&self, i: usize) {
        let Some(tab) = self.tabs.borrow().get(i).cloned() else { return };
        if tab.review_open() {
            tab.0.review.grab_focus();
        } else {
            tab.0.terminal.widget.grab_focus();
        }
        self.window.set_title(&tab.title());
    }

    fn changed(&self, tab: &Tab) {
        if let Some(cur) = self.current() {
            if Rc::ptr_eq(&cur.0, &tab.0) {
                let title = tab.title();
                if self.window.title().as_deref() != Some(title.as_str()) {
                    self.window.set_title(&title);
                }
            }
        }
        if let Some(l) = self.notebook.tab_label(&tab.0.stack).and_then(|w| w.downcast::<gtk::Label>().ok()) {
            l.set_text(&tab.short_title());
        }
        self.dump();
    }

    /// For the tests (TEXTSMITH_TEST_DUMP=file): the state as JSON.
    fn dump(&self) {
        let Some(path) = std::env::var_os("TEXTSMITH_TEST_DUMP") else { return };
        let Some(tab) = self.current() else { return };
        let blocks: Vec<serde_json::Value> = tab
            .0
            .blocks
            .borrow()
            .iter()
            .map(|b| serde_json::json!({ "command": b.command, "status": b.status, "text": b.text, "dir": b.dir }))
            .collect();
        let buf = tab.0.review.buffer().unwrap();
        let (a, b) = buf.bounds();
        let cursor = buf.iter_at_mark(&buf.get_insert().unwrap()).line();
        let focus = self.window.focused_widget().map(|w| w.type_().name().to_string()).unwrap_or_default();
        let v = serde_json::json!({
            "title": self.window.title().map(|t| t.to_string()),
            "tabs": self.tabs.borrow().len(),
            "current": self.notebook.current_page(),
            "state": format!("{:?}", tab.0.state.get()),
            "review_open": tab.review_open(),
            "focus": focus,
            "blocks": blocks,
            "review_text": buf.text(&a, &b, false).map(|s| s.to_string()),
            "review_line": cursor,
            "status": self.status.text().to_string(),
            "cwd": tab.0.cwd.borrow().clone(),
        });
        let tmp = format!("{}.tmp", path.to_string_lossy());
        if std::fs::write(&tmp, v.to_string()).is_ok() {
            let _ = std::fs::rename(tmp, path);
        }
    }

    fn reload_prefs(&self) {
        let t = mtime();
        if t == self.prefs_time.get() {
            return;
        }
        self.prefs_time.set(t);
        let p = Prefs::load();
        for tab in self.tabs.borrow().iter() {
            tab.apply_prefs(&p);
        }
        *self.prefs.borrow_mut() = p;
        if let Some(tab) = self.current() {
            self.window.set_title(&tab.title());
        }
    }

    fn switch(&self, delta: i32) {
        let n = self.tabs.borrow().len() as i32;
        if n < 2 {
            announce(self.window.upcast_ref(), "Only one tab.");
            return;
        }
        let cur = self.notebook.current_page().unwrap_or(0) as i32;
        self.notebook.set_current_page(Some(((cur + delta).rem_euclid(n)) as u32));
    }

    fn key(self: &Rc<Self>, ev: &gdk::EventKey) -> glib::Propagation {
        let Some(combo) = config::combo(ev.keyval(), ev.state()) else { return glib::Propagation::Proceed };
        let p = self.prefs();
        let stop = glib::Propagation::Stop;
        let Some(tab) = self.current() else { return glib::Propagation::Proceed };
        // Keys for the whole window, wherever the focus is.
        if p.is(&combo, "newTab") {
            let cwd = PathBuf::from(tab.0.cwd.borrow().clone());
            self.new_tab(Some(cwd), None);
            return stop;
        }
        if p.is(&combo, "closeTab") {
            self.close_tab(&tab);
            return stop;
        }
        if p.is(&combo, "nextTab") {
            self.switch(1);
            return stop;
        }
        if p.is(&combo, "prevTab") {
            self.switch(-1);
            return stop;
        }
        if p.is(&combo, "preferences") {
            self.open_prefs();
            return stop;
        }
        if p.is(&combo, "manual") {
            self.open_manual();
            return stop;
        }
        if p.is(&combo, "about") {
            self.about();
            return stop;
        }
        if p.is(&combo, "bookmarks") {
            self.bookmark_list(&tab);
            return stop;
        }
        if tab.review_open() {
            if p.is(&combo, "review") || combo == "Escape" {
                tab.close_review();
                return stop;
            }
            if p.is(&combo, "prevCommand") {
                tab.jump(false, false);
                return stop;
            }
            if p.is(&combo, "nextCommand") {
                tab.jump(true, false);
                return stop;
            }
            if p.is(&combo, "prevFailed") {
                tab.jump(false, true);
                return stop;
            }
            if p.is(&combo, "nextFailed") {
                tab.jump(true, true);
                return stop;
            }
            if p.is(&combo, "lastCommand") {
                tab.refresh_review();
                tab.jump_last();
                return stop;
            }
            if p.is(&combo, "copy") {
                if let Some(b) = tab.0.review.buffer() {
                    b.copy_clipboard(&gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD));
                }
                return stop;
            }
            return glib::Propagation::Proceed;
        }
        if p.is(&combo, "review") {
            tab.open_review();
            return stop;
        }
        if p.is(&combo, "copy") {
            if tab.0.terminal.has_selection() {
                tab.0.terminal.copy();
                announce(self.window.upcast_ref(), "Copied.");
            } else {
                announce(self.window.upcast_ref(), "Nothing selected. F4 opens the review buffer, where Shift and the arrows select.");
            }
            return stop;
        }
        if p.is(&combo, "paste") {
            tab.0.terminal.paste();
            return stop;
        }
        // Keys for the shell's own command line: only at a prompt. While a
        // program runs, they go to it as they are.
        if tab.at_prompt() && (combo == "Tab" || combo == "Shift+Tab") && tab.0.terminal.widget.has_focus() {
            return if tab.tab(combo == "Shift+Tab") { stop } else { glib::Propagation::Proceed };
        }
        if tab.at_prompt() && combo == "Enter" && tab.0.terminal.widget.has_focus() {
            return if tab.enter() { stop } else { glib::Propagation::Proceed };
        }
        if tab.at_prompt() {
            let seq: Option<&[u8]> = if p.is(&combo, "bookmarkCall") {
                Some(b"\x1b[9980~")
            } else if p.is(&combo, "bookmarkFill") {
                Some(b"\x1b[9981~")
            } else if p.is(&combo, "sudo") {
                Some(b"\x1b[9982~")
            } else if p.is(&combo, "bookmark") {
                Some(b"\x1b[9983~")
            } else if p.is(&combo, "bookmarkRun") {
                Some(b"\x1b[9984~")
            } else {
                None
            };
            if let Some(s) = seq {
                tab.send(s);
                return stop;
            }
        }
        glib::Propagation::Proceed
    }

    /// F1: the manual that came with this copy, written out as one web page
    /// and opened in the browser, where Orca reads headings and links as on
    /// any site. Without a browser, the manual on the site.
    fn open_manual(&self) {
        let dir = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| crate::store::home().join(".local/share")).join("textsmith");
        let file = dir.join("manual.html");
        let written = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&file, MANUAL));
        let uri = match written {
            Ok(()) => format!("file://{}", file.to_string_lossy()),
            Err(_) => MANUAL_URL.to_string(),
        };
        #[allow(deprecated)]
        if let Err(e) = gtk::show_uri_on_window(Some(&self.window), &uri, gtk::current_event_time()) {
            announce(self.window.upcast_ref(), &format!("Could not open the manual ({e}). It is also at {MANUAL_URL}"));
        }
    }

    /// Ctrl+F1: what The Textsmith is, its version, copyright and licence.
    fn about(&self) {
        let d = gtk::AboutDialog::new();
        d.set_transient_for(Some(&self.window));
        d.set_modal(true);
        d.set_destroy_with_parent(true);
        d.set_program_name("The Textsmith");
        d.set_version(Some(env!("CARGO_PKG_VERSION")));
        d.set_comments(Some(ABOUT));
        d.set_copyright(Some(COPYRIGHT));
        d.set_license_type(gtk::License::Gpl30);
        d.set_website(Some("https://thetextsmith.com"));
        d.set_website_label(Some("thetextsmith.com"));
        d.set_authors(&["Ishe Chinyoka"]);
        d.set_logo_icon_name(Some("utilities-terminal"));
        d.connect_response(|d, _| d.close());
        d.present();
    }

    fn open_prefs(&self) {
        let _ = Prefs::load(); // writes the file with every default if it is not there yet
        let uri = format!("file://{}", config::file().to_string_lossy());
        #[allow(deprecated)]
        match gtk::show_uri_on_window(Some(&self.window), &uri, gtk::current_event_time()) {
            Ok(()) => announce(self.window.upcast_ref(), "Preferences open in your text editor. Save, then come back: they are read again."),
            Err(e) => announce(self.window.upcast_ref(), &format!("Could not open {}: {e}", config::file().display())),
        }
    }

    /// Alt+Shift+B: every bookmark. Enter runs one (or asks for its arguments
    /// on the command line), Ctrl+Enter puts it on the command line to edit,
    /// Delete removes it. Typing filters the list.
    fn bookmark_list(self: &Rc<Self>, tab: &Tab) {
        let dialog = gtk::Dialog::with_buttons(Some("Bookmarks"), Some(&self.window), gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT, &[("Close", gtk::ResponseType::Close)]);
        dialog.set_default_size(640, 460);
        let area = dialog.content_area();
        let search = gtk::SearchEntry::new();
        if let Some(a) = search.accessible() {
            a.set_name("Filter bookmarks");
        }
        let note = gtk::Label::new(Some("Enter runs a bookmark, or puts it on the command line for its arguments; a piped one (|) is added to the end of the line. Ctrl+Enter puts it there to edit. Delete removes it. Typing filters. Escape closes."));
        note.set_line_wrap(true);
        note.set_xalign(0.0);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Browse);
        if let Some(a) = list.accessible() {
            a.set_name("Bookmarks");
        }
        let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scroll.set_vexpand(true);
        scroll.add(&list);
        area.pack_start(&note, false, false, 4);
        area.pack_start(&search, false, false, 4);
        area.pack_start(&scroll, true, true, 4);

        // Each row: a bookmark, and for one with a command per machine while a
        // program runs, the machine chosen.
        let rows: Rc<RefCell<Vec<(String, Option<String>)>>> = Rc::default();
        let remote: Option<Vec<String>> = if tab.at_prompt() { None } else { Some(tab.remote_names().unwrap_or_else(|| vec![bm::machine()])) };
        let running = !tab.at_prompt();
        let fill = {
            let (list, rows, search, remote) = (list.clone(), rows.clone(), search.clone(), remote.clone());
            move || {
                for c in list.children() {
                    list.remove(&c);
                }
                let q = search.text().to_lowercase();
                let mut raws = store::read_lines();
                raws.reverse();
                // Inside a program, a bookmark with a command per machine is one
                // row per machine, the one connected to first.
                let mut entries: Vec<(String, Option<String>, String)> = vec![];
                for raw in &raws {
                    let b = bm::parse(raw);
                    if running && !b.variants.is_empty() {
                        let mut ms: Vec<String> = b.variants.iter().flat_map(|v| v.machines.clone()).collect();
                        let rank = |m: &String| remote.as_ref().and_then(|r| r.iter().position(|x| x == m)).unwrap_or(usize::MAX);
                        ms.sort_by_key(|m| (rank(m), m == "*"));
                        for m in ms {
                            let one = bm::parse_on(raw, &m);
                            let label = if m == "*" { "any other machine".to_string() } else { m.clone() };
                            entries.push((raw.clone(), Some(m), format!("{}, for {label}: {}", b.name, one.this_command())));
                        }
                    } else {
                        let text = if b.name.is_empty() { b.command.clone() } else { format!("{}: {}", b.name, b.command) };
                        entries.push((raw.clone(), None, text));
                    }
                }
                let entries: Vec<_> = entries.into_iter().filter(|e| e.2.to_lowercase().contains(&q)).collect();
                for (raw, m, text) in &entries {
                    let b = match m { Some(m) => bm::parse_on(raw, m), None => bm::parse(raw) };
                    let text = text.clone();
                    let l = gtk::Label::new(Some(&text));
                    l.set_xalign(0.0);
                    l.set_margin_start(6);
                    let row = gtk::ListBoxRow::new();
                    row.add(&l);
                    if let Some(a) = row.accessible() {
                        a.set_name(&text);
                        a.set_description(&bm::describe(&b));
                    }
                    list.add(&row);
                }
                list.show_all();
                if let Some(r) = list.row_at_index(0) {
                    list.select_row(Some(&r));
                }
                *rows.borrow_mut() = entries.into_iter().map(|(r, m, _)| (r, m)).collect();
            }
        };
        fill();
        if rows.borrow().is_empty() {
            note.set_text("No bookmarks yet. Type a command with a label, such as #vps: ssh forge, and press Alt+M.");
        }
        let f = fill.clone();
        search.connect_search_changed(move |_| f());

        let pick = {
            let (dialog, rows, tab, list, app) = (dialog.clone(), rows.clone(), tab.clone(), list.clone(), self.clone());
            let done = Rc::new(Cell::new(false));
            move |run: bool| {
                // Enter in the filter can reach here twice (the entry and the
                // list both answer it): the bookmark is used once.
                if done.replace(true) {
                    return;
                }
                let Some(row) = list.selected_row() else { done.set(false); return };
                let Some((raw, chosen)) = rows.borrow().get(row.index() as usize).cloned() else { return };
                dialog.close();
                if !tab.at_prompt() {
                    // Inside ssh, tmux or any program: the command is typed into
                    // it, without Enter. Over ssh, the command is the one for the
                    // machine connected to.
                    let running = tab.running_command().unwrap_or_default();
                    let b = match &chosen {
                        Some(m) => bm::parse_on(&raw, m),
                        None => bm::parse(&raw),
                    };
                    let prog = running.split_whitespace().next().unwrap_or("the program").to_string();
                    let text = if let Some(e) = &b.missing {
                        announce(app.window.upcast_ref(), e);
                        return;
                    } else if b.wants_args() {
                        b.this_command()
                    } else {
                        match bm::expand(&b, &[]) {
                            Ok(c) => c,
                            Err(e) => {
                                announce(app.window.upcast_ref(), &e);
                                return;
                            }
                        }
                    };
                    let text = if b.pipe { format!(" | {text}") } else { text };
                    tab.send(text.as_bytes());
                    let more = if b.wants_args() { " Replace the places in angle brackets, then Enter." } else { " Enter runs it." };
                    announce(app.window.upcast_ref(), &format!("Typed into {prog}: {text}.{more}"));
                    return;
                }
                let b = bm::parse(&raw);
                if !run {
                    tab.put_line(&bm::format(&b), false);
                } else if b.pipe && !b.name.is_empty() {
                    // Piped into: added to the end of what is on the command line.
                    tab.append_line(&format!("#@{} ", b.name));
                    announce(app.window.upcast_ref(), &format!("Added #@{} to the command line. {}", b.name, bm::describe(&b)));
                } else if !b.name.is_empty() && b.wants_args() {
                    tab.put_line(&format!("#@{} ", b.name), false);
                    announce(app.window.upcast_ref(), &format!("{}. Type its arguments, then Enter.", bm::describe(&b)));
                } else if b.min > 0 {
                    tab.put_line(&b.command, false);
                    announce(app.window.upcast_ref(), "Fill in the places marked with angle brackets, then Enter.");
                } else {
                    match bm::expand(&b, &[]) {
                        Ok(c) => tab.put_line(&c, true),
                        Err(e) => announce(app.window.upcast_ref(), &e),
                    }
                }
            }
        };
        let pk = pick.clone();
        list.connect_row_activated(move |_, _| pk(true));
        let pk = pick.clone();
        search.connect_activate(move |_| pk(true));
        let (pk, list2, rows2, f2) = (pick.clone(), list.clone(), rows.clone(), fill.clone());
        let win = self.window.clone();
        let search2 = search.clone();
        dialog.connect_key_press_event(move |d, ev| {
            let Some(combo) = config::combo(ev.keyval(), ev.state()) else { return glib::Propagation::Proceed };
            match combo.as_str() {
                "Ctrl+Enter" => {
                    pk(false);
                    glib::Propagation::Stop
                }
                "Delete" if list2.has_focus() || list2.focus_child().is_some() => {
                    if let Some(row) = list2.selected_row() {
                        let i = row.index();
                        if let Some((raw, _)) = rows2.borrow().get(i as usize).cloned() {
                            let _ = store::remove(&raw);
                            announce(win.upcast_ref(), &format!("Bookmark removed: {}", bm::parse(&raw).name));
                        }
                        f2();
                        if let Some(r) = list2.row_at_index(i.max(0)).or_else(|| list2.row_at_index(i - 1)) {
                            list2.select_row(Some(&r));
                            r.grab_focus();
                        }
                    }
                    glib::Propagation::Stop
                }
                "Escape" => {
                    d.close();
                    glib::Propagation::Stop
                }
                "Down" if search2.has_focus() => {
                    if let Some(r) = list2.selected_row() {
                        r.grab_focus();
                    }
                    glib::Propagation::Stop
                }
                // Typing anywhere in the list filters it.
                _ if !search2.has_focus() && search2.handle_event(ev) => {
                    search2.grab_focus_without_selecting();
                    search2.set_position(-1);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        let tab2 = tab.clone();
        dialog.connect_response(move |d, _| {
            d.close();
        });
        dialog.connect_destroy(move |_| {
            tab2.0.terminal.widget.grab_focus();
        });
        dialog.show_all();
        match list.selected_row() {
            Some(r) => r.grab_focus(),
            None => search.grab_focus(),
        }
    }
}

/// The machine a running ssh (or mosh) is connected to: the host as typed,
/// without user@. None for anything else.
pub fn remote_machine(command: &str) -> Option<String> {
    let words: Vec<String> = bm::tokenize(command).iter().map(|w| w.trim_matches(|c| c == '\'' || c == '"').to_string()).collect();
    remote_machine_argv(&words)
}

/// The same, from a program's arguments.
pub fn remote_machine_argv(words: &[String]) -> Option<String> {
    let mut it = words.iter().map(String::as_str);
    let first = it.next()?;
    let prog = first.rsplit('/').next().unwrap_or(first);
    if prog != "ssh" && prog != "mosh" && prog != "autossh" {
        return None;
    }
    // ssh options that take a value.
    const WITH_VALUE: &[&str] = &["-b", "-c", "-D", "-E", "-e", "-F", "-I", "-i", "-J", "-L", "-l", "-m", "-O", "-o", "-p", "-Q", "-R", "-S", "-W", "-w", "-M", "--ssh", "--port"];
    while let Some(w) = it.next() {
        if WITH_VALUE.contains(&w) {
            it.next();
        } else if w.starts_with('-') {
            continue;
        } else {
            return Some(w.rsplit('@').next().unwrap_or(w).to_string());
        }
    }
    None
}

/// The login shell from the password database, so chsh applies to the next window.
pub fn login_shell() -> String {
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() && !(*pw).pw_shell.is_null() {
            let s = std::ffi::CStr::from_ptr((*pw).pw_shell).to_string_lossy().into_owned();
            if !s.is_empty() {
                return s;
            }
        }
    }
    std::env::var("SHELL").unwrap_or_else(|_| "bash".into())
}

#[cfg(test)]
mod tests {
    use super::remote_machine;
    #[test]
    fn the_machine_an_ssh_is_connected_to() {
        assert_eq!(remote_machine("ssh forge").as_deref(), Some("forge"));
        assert_eq!(remote_machine("ssh -p 2222 -i ~/.ssh/k ishe@forge.example.com uptime").as_deref(), Some("forge.example.com"));
        assert_eq!(remote_machine("/usr/bin/ssh -A forge").as_deref(), Some("forge"));
        assert_eq!(remote_machine("mosh forge").as_deref(), Some("forge"));
        assert_eq!(remote_machine("tmux").as_deref(), None);
        assert_eq!(remote_machine("vim ssh").as_deref(), None);
    }
}
