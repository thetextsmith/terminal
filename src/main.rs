//! The Textsmith: a Linux terminal for anyone who works with text, commands
//! and the shell.
//!
//! Copyright 2026 Ishe Chinyoka. Distributed under the GNU General Public
//! License, version 3 or later; see LICENSE.
//!
//!   textsmith [--working-directory DIR] [-e command args...]
//!   textsmith --version | --keys | --config-dir | --help
//!   textsmith bm ...          (used by the shell integration)

mod app;
mod bookmarks;
mod config;
mod export;
mod helper;
mod pty;
mod review;
mod scan;
mod session;
mod store;
mod vte;

use std::os::unix::process::CommandExt;
use std::path::PathBuf;

const BASHRC: &str = include_str!("../shell/textsmith.bashrc");
const ZSHENV: &str = include_str!("../shell/zsh/.zshenv");
const ZSHRC: &str = include_str!("../shell/zsh/.zshrc");
const ZPROFILE: &str = include_str!("../shell/zsh/.zprofile");

const HELP: &str = "The Textsmith: a Linux terminal for anyone who works with text, commands and the shell.

  textsmith [--working-directory DIR] [-e command args...]
  textsmith --version      which copy this is
  textsmith --keys         the keys, as set in your preferences
  textsmith --config-dir   the folder with your bookmarks and preferences
  textsmith bm list        your bookmarks with their descriptions
  textsmith bm export -o FILE
                           your bookmarks as shell functions, for other terminals

Starts in the background; --foreground (or TEXTSMITH_FOREGROUND=1) keeps it
attached to the terminal it was started from. F1 in the window opens the manual.

Copyright 2026 Ishe Chinyoka. The Textsmith is distributed under the GNU
General Public License, version 3 or later: https://thetextsmith.com";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bm") => std::process::exit(helper::main(&args[1..])),
        Some("--version") | Some("-v") => {
            let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
            println!("The Textsmith {} ({exe}, VTE {})", env!("CARGO_PKG_VERSION"), vte::version());
            return;
        }
        Some("--config-dir") => {
            println!("{}", store::config_dir().display());
            return;
        }
        Some("--help") | Some("-h") => {
            println!("{HELP}");
            return;
        }
        Some("--keys") => {
            println!("{}", config::key_list(&config::Prefs::load()));
            return;
        }
        _ => {}
    }

    let mut launch = app::Launch { cwd: None, command: None };
    let mut foreground = std::env::var("TEXTSMITH_FOREGROUND").map_or(false, |v| !v.is_empty());
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--working-directory" || a == "-d" {
            launch.cwd = args.get(i + 1).map(PathBuf::from);
            i += 1;
        } else if let Some(d) = a.strip_prefix("--working-directory=") {
            launch.cwd = Some(PathBuf::from(d));
        } else if a == "--foreground" {
            foreground = true;
        } else if a == "-e" || a == "-x" || a == "--command" || a == "--" {
            let rest = &args[i + 1..];
            // One argument is a command line; several are a program and its arguments.
            launch.command = Some(if rest.len() == 1 { rest[0].clone() } else { rest.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" ") });
            break;
        }
        i += 1;
    }

    if !foreground {
        // Detached from wherever it was typed, so it is safe to start from inside itself.
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap_or_else(|_| "textsmith".into()));
        cmd.args(&args).env("TEXTSMITH_FOREGROUND", "1");
        cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        match cmd.spawn() {
            Ok(_) => return,
            Err(e) => eprintln!("textsmith: could not start in the background ({e}); staying here."),
        }
    }
    // Inherited from a terminal that started us: not for our shells.
    std::env::remove_var("TEXTSMITH_FOREGROUND");
    for v in ["TEXTSMITH_TMP", "TEXTSMITH_BIN", "ZDOTDIR_TEXTSMITH", "TEXTSMITH_USER_ZDOTDIR"] {
        std::env::remove_var(v);
    }

    let runtime = runtime_dir();
    let integration = runtime.join(format!("shell-{}", env!("CARGO_PKG_VERSION")));
    if let Err(e) = write_integration(&integration) {
        eprintln!("textsmith: could not write the shell integration to {}: {e}", integration.display());
    }
    app::run(integration, runtime, launch);
}

fn quote(w: &str) -> String {
    if !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || "@%+=:,./-_".contains(c)) {
        w.to_string()
    } else {
        format!("'{}'", w.replace('\'', "'\\''"))
    }
}

fn runtime_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|p| p.is_dir()).unwrap_or_else(|| std::env::temp_dir());
    let uid = unsafe { libc::getuid() };
    let dir = base.join(format!("textsmith-{uid}"));
    let _ = std::fs::create_dir_all(&dir);
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    dir
}

fn write_integration(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("zsh"))?;
    std::fs::write(dir.join("textsmith.bashrc"), BASHRC)?;
    std::fs::write(dir.join("zsh/.zshenv"), ZSHENV)?;
    std::fs::write(dir.join("zsh/.zshrc"), ZSHRC)?;
    std::fs::write(dir.join("zsh/.zprofile"), ZPROFILE)?;
    Ok(())
}
