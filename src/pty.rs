//! A pty and the shell on it. The Textsmith reads the shell's output itself
//! (to see the marks) and hands it to the terminal widget.

use std::cell::RefCell;
use std::ffi::OsString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::rc::Rc;

pub struct Pty {
    pub master: OwnedFd,
    pub pid: i32,
    pending: Rc<RefCell<Vec<u8>>>,
    watching: Rc<RefCell<Option<glib::SourceId>>>,
}

fn winsize(cols: u16, rows: u16) -> libc::winsize {
    libc::winsize { ws_row: rows.max(2), ws_col: cols.max(2), ws_xpixel: 0, ws_ypixel: 0 }
}

impl Pty {
    pub fn spawn(argv: &[OsString], env: &[(String, OsString)], cwd: &Path, cols: u16, rows: u16) -> io::Result<Pty> {
        let (mut m, mut s): (RawFd, RawFd) = (-1, -1);
        let ws = winsize(cols, rows);
        if unsafe { libc::openpty(&mut m, &mut s, std::ptr::null_mut(), std::ptr::null(), &ws) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let master = unsafe { OwnedFd::from_raw_fd(m) };
        let slave = unsafe { OwnedFd::from_raw_fd(s) };
        unsafe {
            libc::fcntl(m, libc::F_SETFD, libc::FD_CLOEXEC);
            let fl = libc::fcntl(m, libc::F_GETFL);
            libc::fcntl(m, libc::F_SETFL, fl | libc::O_NONBLOCK);
        }
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..]).current_dir(cwd);
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::from(slave.try_clone()?)).stdout(Stdio::from(slave.try_clone()?)).stderr(Stdio::from(slave));
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(io::Error::last_os_error());
                }
                if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                // Signals the GTK process ignores should not stay ignored in the shell.
                for sig in [libc::SIGPIPE, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM, libc::SIGCHLD] {
                    libc::signal(sig, libc::SIG_DFL);
                }
                Ok(())
            });
        }
        let child = cmd.spawn()?;
        let pid = child.id() as i32;
        std::mem::forget(child); // reaped by glib's child watch
        Ok(Pty { master, pid, pending: Rc::default(), watching: Rc::default() })
    }

    pub fn fd(&self) -> RawFd {
        self.master.as_raw_fd()
    }

    /// The arguments of the program in the foreground on the pty, as it was
    /// really started (through an alias or a bookmark, it is still `ssh forge`).
    pub fn foreground_argv(&self) -> Option<Vec<String>> {
        let pg = unsafe { libc::tcgetpgrp(self.fd()) };
        if pg <= 0 {
            return None;
        }
        let raw = std::fs::read(format!("/proc/{pg}/cmdline")).ok()?;
        let argv: Vec<String> = raw.split(|b| *b == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
        if argv.is_empty() { None } else { Some(argv) }
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        let ws = winsize(cols, rows);
        unsafe { libc::ioctl(self.fd(), libc::TIOCSWINSZ, &ws) };
    }

    /// Read what is there without blocking. None: the shell has gone.
    pub fn read(&self, buf: &mut [u8]) -> Option<usize> {
        let r = unsafe { libc::read(self.fd(), buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if r > 0 {
            return Some(r as usize);
        }
        if r < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::Interrupted {
                return Some(0);
            }
        }
        None
    }

    /// Write to the shell. What does not fit now is sent when the pty has room.
    pub fn write(&self, data: &[u8]) {
        self.pending.borrow_mut().extend_from_slice(data);
        Self::drain(self.fd(), &self.pending);
        if !self.pending.borrow().is_empty() && self.watching.borrow().is_none() {
            let (pending, watching, fd) = (self.pending.clone(), self.watching.clone(), self.fd());
            let id = glib::source::unix_fd_add_local(fd, glib::IOCondition::OUT | glib::IOCondition::ERR | glib::IOCondition::HUP, move |_, cond| {
                if cond.intersects(glib::IOCondition::ERR | glib::IOCondition::HUP) {
                    pending.borrow_mut().clear();
                }
                Self::drain(fd, &pending);
                if pending.borrow().is_empty() {
                    watching.borrow_mut().take();
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
            *self.watching.borrow_mut() = Some(id);
        }
    }

    fn drain(fd: RawFd, pending: &RefCell<Vec<u8>>) {
        let mut p = pending.borrow_mut();
        while !p.is_empty() {
            let r = unsafe { libc::write(fd, p.as_ptr() as *const libc::c_void, p.len()) };
            if r > 0 {
                p.drain(..r as usize);
                continue;
            }
            let e = io::Error::last_os_error();
            if r < 0 && e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if r < 0 && e.kind() != io::ErrorKind::WouldBlock {
                p.clear();
            }
            break;
        }
    }

    pub fn hangup(&self) {
        unsafe { libc::kill(-self.pid, libc::SIGHUP) };
        unsafe { libc::kill(self.pid, libc::SIGHUP) };
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        if let Some(id) = self.watching.borrow_mut().take() {
            id.remove();
        }
    }
}
