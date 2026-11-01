//! The few parts of libvte-2.91 (GTK 3) The Textsmith needs, bound by hand.
//! The terminal widget has no pty of its own: The Textsmith feeds it what the
//! shell writes and sends on what it types (the "commit" signal), so it can
//! see the shell integration's marks in order.

use glib::translate::*;
use gtk::prelude::*;
use std::ffi::{c_char, c_long, c_void, CStr};

#[repr(C)]
pub struct VteTerminalRaw {
    _private: [u8; 0],
}

#[link(name = "vte-2.91")]
extern "C" {
    fn vte_terminal_new() -> *mut gtk::ffi::GtkWidget;
    fn vte_terminal_feed(t: *mut VteTerminalRaw, data: *const c_char, len: isize);
    fn vte_terminal_set_scrollback_lines(t: *mut VteTerminalRaw, lines: c_long);
    fn vte_terminal_get_cursor_position(t: *mut VteTerminalRaw, col: *mut c_long, row: *mut c_long);
    fn vte_terminal_get_text_range_format(
        t: *mut VteTerminalRaw,
        format: i32,
        start_row: c_long,
        start_col: c_long,
        end_row: c_long,
        end_col: c_long,
        length: *mut usize,
    ) -> *mut c_char;
    fn vte_terminal_get_column_count(t: *mut VteTerminalRaw) -> c_long;
    fn vte_terminal_get_row_count(t: *mut VteTerminalRaw) -> c_long;
    fn vte_terminal_set_font(t: *mut VteTerminalRaw, font: *const pango::ffi::PangoFontDescription);
    fn vte_terminal_copy_clipboard_format(t: *mut VteTerminalRaw, format: i32);
    fn vte_terminal_paste_clipboard(t: *mut VteTerminalRaw);
    fn vte_terminal_get_has_selection(t: *mut VteTerminalRaw) -> glib::ffi::gboolean;
    fn vte_terminal_set_colors(
        t: *mut VteTerminalRaw,
        fg: *const gdk::ffi::GdkRGBA,
        bg: *const gdk::ffi::GdkRGBA,
        palette: *const gdk::ffi::GdkRGBA,
        size: usize,
    );
    fn vte_terminal_set_mouse_autohide(t: *mut VteTerminalRaw, setting: glib::ffi::gboolean);
    fn vte_terminal_set_scroll_on_keystroke(t: *mut VteTerminalRaw, setting: glib::ffi::gboolean);
    fn vte_terminal_set_scroll_on_output(t: *mut VteTerminalRaw, setting: glib::ffi::gboolean);
    fn vte_terminal_set_audible_bell(t: *mut VteTerminalRaw, setting: glib::ffi::gboolean);
    fn vte_get_major_version() -> u32;
    fn vte_get_minor_version() -> u32;
    fn vte_get_micro_version() -> u32;
}

const FORMAT_TEXT: i32 = 1;

/// A VteTerminal, held as an ordinary GTK widget.
#[derive(Clone)]
pub struct Terminal {
    pub widget: gtk::Widget,
}

pub fn version() -> String {
    unsafe { format!("{}.{}.{}", vte_get_major_version(), vte_get_minor_version(), vte_get_micro_version()) }
}

impl Terminal {
    pub fn new() -> Terminal {
        let widget: gtk::Widget = unsafe { from_glib_none(vte_terminal_new()) };
        let t = Terminal { widget };
        unsafe {
            vte_terminal_set_mouse_autohide(t.raw(), 1);
            vte_terminal_set_scroll_on_keystroke(t.raw(), 1);
            vte_terminal_set_scroll_on_output(t.raw(), 0);
        }
        t
    }

    fn raw(&self) -> *mut VteTerminalRaw {
        self.widget.as_ptr() as *mut VteTerminalRaw
    }

    pub fn feed(&self, data: &[u8]) {
        if !data.is_empty() {
            unsafe { vte_terminal_feed(self.raw(), data.as_ptr() as *const c_char, data.len() as isize) }
        }
    }

    pub fn set_scrollback(&self, lines: i64) {
        unsafe { vte_terminal_set_scrollback_lines(self.raw(), lines as c_long) }
    }

    /// (column, row), the row counted from the start of the scrollback.
    pub fn cursor(&self) -> (i64, i64) {
        let (mut c, mut r) = (0, 0);
        unsafe { vte_terminal_get_cursor_position(self.raw(), &mut c, &mut r) };
        (c as i64, r as i64)
    }

    /// The text between two positions, lines joined as they were written.
    pub fn text(&self, start: (i64, i64), end: (i64, i64)) -> String {
        let mut len = 0usize;
        unsafe {
            let p = vte_terminal_get_text_range_format(self.raw(), FORMAT_TEXT, start.1 as c_long, start.0 as c_long, end.1 as c_long, end.0 as c_long, &mut len);
            if p.is_null() {
                return String::new();
            }
            let s = CStr::from_ptr(p).to_string_lossy().into_owned();
            glib::ffi::g_free(p as *mut c_void);
            s
        }
    }

    /// How many lines the scrollback keeps.
    pub fn scrollback(&self) -> i64 {
        self.widget.property::<u32>("scrollback-lines") as i64
    }

    pub fn size(&self) -> (u16, u16) {
        unsafe { (vte_terminal_get_column_count(self.raw()) as u16, vte_terminal_get_row_count(self.raw()) as u16) }
    }

    pub fn set_font(&self, font: &pango::FontDescription) {
        unsafe { vte_terminal_set_font(self.raw(), font.to_glib_none().0) }
    }

    pub fn copy(&self) {
        unsafe { vte_terminal_copy_clipboard_format(self.raw(), FORMAT_TEXT) }
    }
    pub fn paste(&self) {
        unsafe { vte_terminal_paste_clipboard(self.raw()) }
    }
    pub fn has_selection(&self) -> bool {
        unsafe { vte_terminal_get_has_selection(self.raw()) != 0 }
    }
    pub fn set_audible_bell(&self, on: bool) {
        unsafe { vte_terminal_set_audible_bell(self.raw(), on as i32) }
    }

    pub fn set_colors(&self, fg: &gdk::RGBA, bg: &gdk::RGBA, palette: &[gdk::RGBA]) {
        let pal: Vec<gdk::ffi::GdkRGBA> = palette.iter().map(|c| unsafe { *c.to_glib_none().0 }).collect();
        unsafe {
            vte_terminal_set_colors(
                self.raw(),
                fg.to_glib_none().0,
                bg.to_glib_none().0,
                if pal.is_empty() { std::ptr::null() } else { pal.as_ptr() },
                pal.len(),
            )
        }
    }

    /// What the terminal sends: typed keys, pastes, and replies to queries.
    pub fn connect_commit<F: Fn(&[u8]) + 'static>(&self, f: F) {
        self.widget.connect_local("commit", false, move |args| {
            let v: *const glib::gobject_ffi::GValue = args[1].to_glib_none().0;
            let ptr: *const c_char = unsafe { glib::gobject_ffi::g_value_get_string(v) };
            let n: u32 = args[2].get().unwrap_or(0);
            if !ptr.is_null() {
                let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, n as usize) };
                f(bytes);
            }
            None
        });
    }
}
