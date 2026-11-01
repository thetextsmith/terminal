//! Preferences: gtk-prefs.json in The Textsmith's folder, apart from the Electron
//! version's prefs.json so that neither program drops the other's settings.
//! Written with every default on first start, so it can be edited by hand;
//! The Textsmith reads it again when its window gets the focus back.

use serde_json::{json, Map, Value};
use std::fs;
use std::path::PathBuf;

/// Every action with a key, its default, and what it does.
pub const KEYS: &[(&str, &str, &str)] = &[
    ("review", "F4", "Open the review buffer, and back to the terminal"),
    ("bookmarkCall", "Alt+Enter", "Run the line as a bookmark call: name and arguments"),
    ("bookmarkFill", "Alt+Shift+Enter", "Fill in the bookmark call without running it"),
    ("sudo", "Shift+Enter", "Run the line with sudo"),
    ("bookmark", "Alt+M", "Bookmark the line (or the last command), or remove that bookmark"),
    ("bookmarkRun", "Alt+Shift+M", "Bookmark the line and run it"),
    ("bookmarks", "Alt+Shift+B", "List bookmarks"),
    ("copy", "Ctrl+Shift+C", "Copy the selection"),
    ("paste", "Ctrl+Shift+V", "Paste"),
    ("newTab", "Ctrl+Shift+T", "New tab"),
    ("closeTab", "Ctrl+Shift+W", "Close tab"),
    ("nextTab", "Ctrl+PageDown or Ctrl+Tab", "Next tab"),
    ("prevTab", "Ctrl+PageUp or Ctrl+Shift+Tab", "Previous tab"),
    ("preferences", "Ctrl+Comma", "Open the preferences file"),
    ("manual", "F1", "Open the manual in your web browser"),
    ("about", "Ctrl+F1", "About The Textsmith: version, copyright and licence"),
    ("prevCommand", "Alt+Up", "Review: previous command"),
    ("nextCommand", "Alt+Down", "Review: next command"),
    ("prevFailed", "Alt+PageUp", "Review: previous command that failed"),
    ("nextFailed", "Alt+PageDown", "Review: next command that failed"),
    ("lastCommand", "Alt+End", "Review: the last command"),
];

pub fn defaults() -> Value {
    let keys: Map<String, Value> = KEYS.iter().map(|(k, v, _)| (k.to_string(), json!(v))).collect();
    json!({
        "display": { "font": "Monospace 12", "scrollback": 100000, "colours": "system", "bell": true },
        "title": { "format": "{dir}", "running": "{dir}" },
        "startup": { "directory": "", "shell": "" },
        "editing": { "tabCompletion": "cycle" },
        "review": { "command": "$ {command}{status}", "speakJumps": false },
        "keys": keys,
        "meta": { "version": 4 }
    })
}

pub fn file() -> PathBuf {
    crate::store::config_dir().join("gtk-prefs.json")
}

#[derive(Clone)]
pub struct Prefs(pub Value);

impl Prefs {
    /// Read the file; anything missing or of the wrong type takes the default.
    pub fn load() -> Prefs {
        let def = defaults();
        let saved: Value = fs::read_to_string(file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        let mut out = def.clone();
        if let (Some(o), Some(s)) = (out.as_object_mut(), saved.as_object()) {
            for (group, dvals) in def.as_object().unwrap() {
                let Some(sg) = s.get(group).and_then(Value::as_object) else { continue };
                let og = o.get_mut(group).unwrap().as_object_mut().unwrap();
                for (k, dv) in dvals.as_object().unwrap() {
                    if let Some(v) = sg.get(k) {
                        let same = matches!((v, dv), (Value::String(_), Value::String(_)) | (Value::Bool(_), Value::Bool(_)) | (Value::Number(_), Value::Number(_)));
                        if same {
                            og.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
        }
        // Version 2: the title became just the directory. A title still on the
        // old default moves to the new one; one you changed stays.
        let ver = saved["meta"]["version"].as_i64().unwrap_or(0);
        if ver < 2 {
            let t = out["title"].as_object_mut().unwrap();
            if t.get("format").and_then(Value::as_str) == Some("{prompt} — {shell} — {app}") {
                t.insert("format".into(), json!("{dir}"));
            }
            if t.get("running").and_then(Value::as_str) == Some("{command} — {shell} — {app}") {
                t.insert("running".into(), json!("{dir}"));
            }
        }
        // Version 3: GTK already tells Orca where the cursor goes, so jumps in
        // the review are no longer announced on top of that.
        if ver < 3 {
            out["review"]["speakJumps"] = json!(false);
        }
        // Version 4: Alt+Shift+arrows are Orca's table keys; the failed-command
        // jumps move to Alt+PageUp and Alt+PageDown, unless you chose your own.
        if ver < 4 {
            for (k, old, new) in [("prevFailed", "Alt+Shift+Up", "Alt+PageUp"), ("nextFailed", "Alt+Shift+Down", "Alt+PageDown")] {
                if out["keys"][k].as_str() == Some(old) {
                    out["keys"][k] = json!(new);
                }
            }
        }
        if ver < 4 {
            out["meta"]["version"] = json!(4);
            if file().exists() {
                let _ = fs::write(file(), serde_json::to_string_pretty(&out).unwrap() + "\n");
            }
        }
        if !file().exists() {
            let _ = fs::create_dir_all(crate::store::config_dir());
            let _ = fs::write(file(), serde_json::to_string_pretty(&out).unwrap() + "\n");
        }
        Prefs(out)
    }

    pub fn s(&self, group: &str, key: &str) -> String {
        self.0[group][key].as_str().unwrap_or("").to_string()
    }
    pub fn b(&self, group: &str, key: &str) -> bool {
        self.0[group][key].as_bool().unwrap_or(false)
    }
    pub fn n(&self, group: &str, key: &str) -> i64 {
        self.0[group][key].as_i64().unwrap_or(0)
    }

    /// Does this key press (a combination name such as "Ctrl+Shift+T") belong to the action?
    pub fn is(&self, combo: &str, action: &str) -> bool {
        matches(combo, &self.s("keys", action))
    }
}

/// "F2 or Alt+R" holds two combinations.
pub fn matches(combo: &str, setting: &str) -> bool {
    !combo.is_empty() && setting.split(" or ").map(str::trim).any(|s| s.eq_ignore_ascii_case(combo))
}

/// A key press as a combination name: "Ctrl+Shift+T", "Alt+Enter", "F4".
/// None for a lone modifier.
pub fn combo(keyval: gdk::keys::Key, state: gdk::ModifierType) -> Option<String> {
    let raw = keyval.name()?.to_string();
    if raw.starts_with("Shift_") || raw.starts_with("Control_") || raw.starts_with("Alt_") || raw.starts_with("Super_") || raw.starts_with("Meta_") || raw == "ISO_Level3_Shift" || raw.starts_with("Hyper_") {
        return None;
    }
    let name = match raw.as_str() {
        "Return" | "KP_Enter" | "ISO_Enter" => "Enter".to_string(),
        "Page_Down" | "Next" | "KP_Page_Down" | "KP_Next" => "PageDown".into(),
        "Page_Up" | "Prior" | "KP_Page_Up" | "KP_Prior" => "PageUp".into(),
        "ISO_Left_Tab" => "Tab".into(),
        "KP_Up" => "Up".into(),
        "KP_Down" => "Down".into(),
        "KP_Home" => "Home".into(),
        "KP_End" => "End".into(),
        "comma" => "Comma".into(),
        "period" => "Period".into(),
        "space" => "Space".into(),
        "minus" => "Minus".into(),
        "plus" | "equal" => "Equal".into(),
        "slash" => "Slash".into(),
        "BackSpace" => "Backspace".into(),
        s if s.chars().count() == 1 => s.to_uppercase(),
        s => s.to_string(),
    };
    let mut parts = vec![];
    if state.contains(gdk::ModifierType::CONTROL_MASK) {
        parts.push("Ctrl");
    }
    if state.contains(gdk::ModifierType::MOD1_MASK) {
        parts.push("Alt");
    }
    if state.contains(gdk::ModifierType::SHIFT_MASK) {
        parts.push("Shift");
    }
    if state.contains(gdk::ModifierType::SUPER_MASK) || state.contains(gdk::ModifierType::META_MASK) {
        parts.push("Meta");
    }
    parts.push(&name);
    Some(parts.join("+"))
}

/// The keys as a readable list, for the README and `--keys`.
pub fn key_list(p: &Prefs) -> String {
    KEYS.iter().map(|(k, _, what)| format!("{:<28} {}", p.s("keys", k), what)).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_with_alternatives() {
        assert!(matches("Ctrl+Tab", "Ctrl+PageDown or Ctrl+Tab"));
        assert!(matches("alt+enter", "Alt+Enter"));
        assert!(!matches("Alt+Enter", "Alt+Shift+Enter"));
        assert!(!matches("", ""));
    }
}
