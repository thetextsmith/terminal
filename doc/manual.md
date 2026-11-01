---
title: The Textsmith manual
subtitle: Version 1.0
---

## Introduction

The Textsmith is a Linux terminal for anyone who works with text, commands
and the shell. It is built on GTK 3 and VTE, the same terminal widget as
GNOME Terminal and Terminator, so your prompt, readline, Tab completion and
full-screen programs such as vim, htop and mutt behave as they always have.

On top of an ordinary terminal, The Textsmith adds three things:

- **Bookmarks.** Save a command once, with places for its arguments, and run
  it by name: `burn arch.iso`, `send Music`, `update`. Bookmarks can have
  default values, take any number of arguments, run a different command on
  each of your machines, and be exported as ordinary shell functions.
- **The review buffer.** Press F4 and every command of the session is
  there with its output, as plain text to read, search and copy. Alt+Up and
  Alt+Down jump from command to command, Alt+PageUp and Alt+PageDown to the
  ones that failed.
- **A quiet terminal for screen readers.** The Textsmith was written with
  Orca from the first line. Enter and Tab do not make Orca read your prompt
  again; Tab says each completion whole; and the program only speaks up when
  a key would otherwise give no answer.

It is free software, distributed under the GNU General Public License,
version 3. Copyright 2026 Ishe Chinyoka.

### Two editions

**The Textsmith** (this manual) is the main edition: a native GTK
terminal, written in Rust.

**The Textsmith Electron edition** (`textsmith-electron`) is a different
experience of the same idea: the session is a web page, each command a
heading and its output the text under it, read as you would read a page in
Firefox. It shares the bookmarks file, so both editions can be used side by
side. Its own README describes its keys.

## Installing

The Textsmith runs on Linux, under X11 and Wayland. It needs GTK 3 and VTE
(2.91), which most desktops already have.

### Debian, Ubuntu and Linux Mint

Download `textsmith_1.0.0-1_amd64.deb` from the download page and install
it:

    sudo apt install ./textsmith_1.0.0-1_amd64.deb

The package is built for Ubuntu 24.04, Linux Mint 22, Debian 13 and later.
It pulls in VTE and GTK if they are missing.

### Arch Linux

Build the package with the `PKGBUILD` from the download page:

    sudo pacman -S --needed base-devel
    mkdir textsmith && cd textsmith
    # put the PKGBUILD here, then:
    makepkg -si

### Any other distribution: from the source

Install Rust (from your distribution or rustup.rs) and the VTE and GTK 3
development files (`vte3` and `gtk3` on Arch, `libvte-2.91-dev` and
`libgtk-3-dev` on Debian and Ubuntu, `vte291-devel` and `gtk3-devel` on
Fedora). Then, in the unpacked source folder:

    sh scripts/install.sh

This builds The Textsmith, puts `textsmith` in `~/.local/bin` and adds it
to your application menu. The source download also holds a prebuilt
`bin/textsmith`, which `install.sh` uses when Rust is not installed.

### Making it your terminal

- **GNOME:** Settings, Keyboard, Custom Shortcuts: add one that runs
  `textsmith` on Super+Enter, Ctrl+Alt+T or the key you like. Recent GNOME
  and KDE also read `~/.config/xdg-terminals.list`:
  `echo textsmith.desktop >> ~/.config/xdg-terminals.list`
- **MATE:** `gsettings set org.mate.applications-terminal exec textsmith`
- **Cinnamon:**
  `gsettings set org.cinnamon.desktop.default-applications.terminal exec textsmith`
- **i3 or sway:** `bindsym $mod+Return exec textsmith`
- Many programs also honour `export TERMINAL=textsmith` in `~/.profile`.

`textsmith` starts in the background, so it is safe to start from inside
itself. It understands `--working-directory DIR` and `-e command`, which
file managers use for "open a terminal here".

### Checking the installation

    textsmith --version

says which copy is running. `textsmith --keys` lists your keys, and
`textsmith --config-dir` says where your bookmarks and preferences live.

## First steps

Start The Textsmith from your application menu or with your shortcut. You
get your usual shell, bash or zsh, with your own `~/.bashrc` or
`~/.zshrc` read first. The Textsmith then adds its own small hooks, which
make bookmarks and the review buffer work. Other shells start bash.

Everything you know still works: Ctrl+R searches history, Tab completes,
Ctrl+C stops a program. The Textsmith adds a handful of keys of its own
(see [Keys](#keys)), and only at the shell's prompt: while a program runs,
every key goes to it unchanged.

Tabs: Ctrl+Shift+T opens one in the same directory, Ctrl+Shift+W closes
it, Ctrl+PageDown and Ctrl+PageUp move between them. The window title is
the current directory.

Press **F1** at any time for this manual and **Ctrl+F1** for the About
window.

## Bookmarks

A bookmark is a command you save once and run by name. The Textsmith keeps
them in one plain text file, `bookmarks`, one per line, which you can edit
by hand and carry between machines.

### Making a bookmark

Type the command as a comment, with a name and a colon in front, then
press **Alt+M**:

    #vps: ssh forge

The line is saved as a bookmark called `vps`. Alt+M on a line without a
label bookmarks it as it is; on an empty line, it bookmarks the last
command you ran. Pressing Alt+M on a bookmark that is already there removes
it. **Alt+Shift+M** saves the bookmark and runs it at once.

A label can carry a description, which the bookmark list shows:

    #vps my server: ssh forge

### Running a bookmark

There are three ways, all at your shell's prompt:

1. **Alt+Enter.** Type the name and its arguments as if it were a command,
   `vps`, and press Alt+Enter instead of Enter.
2. **`#@`.** Type `#@vps` and press Enter. Tab after `#@` completes the
   names and says what each bookmark takes.
3. **The bookmark list.** Alt+Shift+B lists every bookmark. Type to filter
   it, then Enter to run one. A bookmark that needs arguments is put on the
   command line for you to complete. Ctrl+Enter puts it there to edit;
   Delete removes it.

Whichever way you use, the command line first shows what will actually run,
and that is what goes into your history. A mistake, such as an unknown name
or a missing argument, is said on a line of its own and your line stays as
it was. **Alt+Shift+Enter** fills a call in without running it, so you can
read the command before pressing Enter.

### Places for arguments

`<>` or `<name>` marks where an argument goes:

    #copy: cp <src> <dst>
    copy a.txt backup/                runs  cp a.txt backup/

Arguments fill the places in the order they first appear, and are put in
exactly as you typed them, quotes and all. A name used twice gets the same
value both times. To write a real `<>` in a command, type `\<>`.

### Default values

`<name:default>` is a place that fills itself in when you leave it out:

    #burn: dd if=<iso> of=<disk:/dev/sdb> bs=1M status=progress
    burn arch.iso                     runs  dd if=arch.iso of=/dev/sdb ...
    burn arch.iso /dev/sdc            runs  dd if=arch.iso of=/dev/sdc ...

Arguments fill places in order, so a default only helps when every place
after it has a default too. A default can hold spaces and quotes, but not
`>`.

### A default taken from another place

When two arguments are usually the same, but not always, let one default
to the other:

    #send: rclone sync <from> drive:Library/<to:<from>> --progress
    send Music                        runs  rclone sync Music drive:Library/Music --progress
    send Music Songs                  runs  rclone sync Music drive:Library/Songs --progress

`<to:<from>>` reads "to, which is from unless you give it". The default can
add to the other place: `#bk: cp <file> <to:<file>.bak>` makes `bk notes.txt`
copy to `notes.txt.bak`. When two values are always the same, use one name
twice instead: `<dir> ... <dir>`.

### Any number of arguments

`<..>` (or `<name..>`) takes every argument the other places leave over,
none included:

    #install: sudo apt install -y <..>
    install vim git htop              runs  sudo apt install -y vim git htop
    #cpto: cp -v <..> <dst>
    cpto a.txt b.txt backup/          runs  cp -v a.txt b.txt backup/

A bookmark has one such place (the same name used again repeats it), and it
can have a default: `<opts..:--all-targets>`.

### Counting arguments

A count in the label says how many arguments a bookmark takes. Without
places, the arguments are added at the end:

    #copy(2..): cp -v                 at least two
    #l(..1): ls -lF                   at most one
    #install(1..): sudo apt install -y <..>   at least one package

### Bookmarks at the end of a line

`#@` at the end of a command adds a bookmark to it, much like a global alias
in zsh. A bookmark whose label starts with `#|` is piped into:

    #|count: wc -l
    #|grab: grep -i <>
    ls -1 #@count                     runs  ls -1 | wc -l
    find . #@grab todo @count         runs  find . | grep -i todo | wc -l

After the first `#@`, further bookmarks only need `@`.

### The bookmarks file

The file is called `bookmarks`, in The Textsmith's folder (see
[Preferences](#preferences)). Each line is a bookmark exactly as you would
type it, and lines that start with `# ` (a hash and a space) are notes:

    # Servers
    #vps my server: ssh forge
    #site: rsync -rltvP --delete <source> forge:/var/www/<destination>

Edit it with any text editor; The Textsmith reads it again each time it is
used. The Electron edition reads the same file.

## A command per machine

When your bookmarks travel between machines, in your dotfiles for instance,
one name can stand for a different command on each:

    #update: [:anvil "sudo apt update && sudo apt upgrade -y", :brainpower "sudo pacman -Syu", :hammer "sudo dnf update"]

`update` then runs the command for the machine it is on. Several machines
can share a command (`:anvil :forge "..."`), and `:*` stands for any other
machine. Each command can have places, defaults and `<..>` of its own.
Commands go in double quotes (with `\"` for a quote inside) or in single
quotes. On a machine with no command and no `:*`, the bookmark says so and
runs nothing.

The machine's name is the first of these that is set:

1. the `TEXTSMITH_MACHINE` environment variable;
2. a file called `machine` in The Textsmith's folder, holding one line
   (keep it out of your dotfiles: it is what makes this machine different);
3. the host name, from `/etc/hostname`, with nothing to set up.

A bookmark that belongs to one machine only needs no list at all:
`#update: sudo pacman -Syu`.

## Inside ssh, tmux and other programs

Bookmarks run at your own shell's prompt. Inside `ssh`, `tmux` or any other
program, the shell is not The Textsmith's, so `#@`, Alt+Enter and Alt+M do
not apply there. The bookmark list does: while a program runs, choosing a
bookmark types its command into the program without pressing Enter, so you
can hear it and then press Enter yourself. A bookmark that takes arguments
is typed with its places (`<file>`) for you to replace.

Inside `ssh forge`, a bookmark with a command per machine shows one row per
machine ("update, for forge: ..."), with the machine you are connected to
first. The Textsmith finds it from the ssh that is really running, however
you started it (an alias, a bookmark), and matches the name you gave ssh or
the host name from your `~/.ssh/config`.

## Bookmarks in other terminals

Bookmarks need not stay in The Textsmith. `textsmith bm export` writes
them as ordinary bash and zsh functions:

    textsmith bm export -o ~/.config/textsmith/bookmarks.sh

Then, in `~/.bashrc` or `~/.zshrc` on any machine or terminal:

    [ -f ~/.config/textsmith/bookmarks.sh ] && . ~/.config/textsmith/bookmarks.sh

Each bookmark becomes a function of the same name. It takes the same
arguments and says the same sentence when they are wrong:

    site() {
      (( $# == 2 )) || { printf '%s\n' 'site takes 2 arguments: source, destination; you gave '"$#." >&2; return 2; }
      rsync -rltvP --delete "${1}" forge:/var/www/"${2}"
    }

Defaults, `<..>`, counts and commands per machine all carry over. A piped
bookmark becomes a function to pipe into: `ls | count`. Bookmarks without a
name, and additions that are not commands on their own (`#quiet:
2>/dev/null`), are left out. Run the export again after changing your
bookmarks.

A function hides any command of the same name, so a bookmark named after a
builtin or a program on your PATH (`install`, say) is left out, and the
export tells you. Rename the bookmark, or give every function a prefix with
`--prefix ts_` (`ts_install`, `ts_site`). `--keep-clashes` exports them
anyway.

## The review buffer

**F4** opens a read-only view of the tab's session: each command on a line
of its own, `$ make (failed, exit 2)`, and its output under it. The cursor
starts on the last command.

- The arrow keys and the usual text keys read it. Shift with them selects,
  and Ctrl+C copies.
- Alt+Up and Alt+Down: the previous and next command.
- Alt+PageUp and Alt+PageDown: the previous and next command that failed.
- Alt+End: the last command, with the newest output.
- Escape or F4: back to the terminal.

The output is taken when each command finishes, so progress bars show as
they ended. Full-screen programs such as vim are not in the review, only
what they left on the normal screen.

## Keys

| Key | What it does |
| --- | --- |
| F1 | This manual, in your web browser |
| Ctrl+F1 | About The Textsmith |
| F4 | The review buffer, and back |
| Alt+Enter | Run the line as a bookmark call |
| Alt+Shift+Enter | Fill the bookmark call in without running it |
| Shift+Enter | Run the line with sudo |
| Alt+M | Bookmark the line (or the last command), or remove that bookmark |
| Alt+Shift+M | Bookmark the line and run it |
| Alt+Shift+B | The bookmark list |
| Tab, Shift+Tab | Complete: the next and previous choice |
| Ctrl+Shift+C, Ctrl+Shift+V | Copy, paste |
| Ctrl+Shift+T, Ctrl+Shift+W | New tab, close tab |
| Ctrl+PageDown, Ctrl+PageUp | Next tab, previous tab |
| Ctrl+Comma | Open the preferences file |

In the review buffer: Alt+Up, Alt+Down, Alt+PageUp, Alt+PageDown and
Alt+End, as above.

Every key can be changed in the preferences. The keys for the command line
only act at the shell's prompt, so Alt+M still reaches emacs and
Shift+Enter is just Enter in python.

**Tab** goes through the choices, as zsh does: when several things match,
the first Tab puts in the first one, each Tab after it the next, and
Shift+Tab goes back. Each choice is said whole. Set
`editing.tabCompletion` to `"shell"` for bash's usual listing instead.

## Preferences

The Textsmith keeps your bookmarks and preferences in one folder, the first
of these that applies:

1. `TEXTSMITH_CONFIG` (or `TEXTSMITH_HOME`), if you set it;
2. `~/.textsmith`, if it is there (earlier versions used it);
3. `~/.config/textsmith`, or `$XDG_CONFIG_HOME/textsmith`.

`textsmith --config-dir` says which one is in use.

The preferences are in `gtk-prefs.json`, written with every default the
first time The Textsmith starts. **Ctrl+Comma** opens it in your text
editor, and it is read again when the window gets the focus back.

- `display`: `font` ("Monospace 12"), `scrollback` (lines kept),
  `colours` ("system" follows your desktop; or "dark", "light",
  "high-contrast"), and `bell`.
- `title`: `format` at the prompt and `running` while a command runs.
  Both are `{dir}`, the directory. Other fields: `{prompt}`, `{shell}`,
  `{command}`, `{app}`.
- `startup`: `directory`, where new tabs start, and `shell`.
- `editing`: `tabCompletion`, `"cycle"` or `"shell"`.
- `review`: `command`, how a command line looks in the review, and
  `speakJumps`, `true` to have each jump announced.
- `keys`: every key in the table above. A key may hold alternatives:
  `"Ctrl+PageDown or Ctrl+Tab"`.

## With a screen reader

The Textsmith is tested with Orca on GNOME and MATE. Some notes:

- Typing, the cursor and new output are read by Orca as in GNOME
  Terminal. Enter and Tab do not make it read your prompt again.
- For long output, F4 is the review buffer: a text view, where Orca's
  usual reading keys work, with jumps between commands.
- Alt+Shift with the arrows are Orca's table keys, so The Textsmith leaves
  them alone; failed commands are on Alt+PageUp and Alt+PageDown.
- When a key has nothing to report ("No earlier command", a bookmark
  mistake), The Textsmith says so through the accessibility layer, once.

## Help and feedback

- The site, with this manual and news: <https://thetextsmith.com>
- Under the Anvil, the blog about working with text and the shell:
  <https://thetextsmith.com/blog/>
- Questions, bugs and ideas: the issue tracker,
  <https://github.com/thetextsmith/textsmith/issues>.

Copyright 2026 Ishe Chinyoka. The Textsmith is free software: you can
redistribute it and change it under the terms of the GNU General Public
License, version 3 or (at your option) any later version. It comes with no
warranty. The full licence is in the `LICENSE` file and in the About
window.
