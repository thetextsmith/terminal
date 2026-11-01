# The Textsmith

The Textsmith is a powerful Linux terminal boosting productivity for anyone
who works with text, commands and the shell. It is an ordinary terminal (GTK 3
and VTE, the widget inside GNOME Terminal and Terminator), written with Orca
from the first line. Your prompt, readline and programs behave exactly as in
any VTE terminal. On top of that:

- **Bookmarks**: a command saved once, with places for its arguments, run
  by name. Places can have **default values** (`<disk:/dev/sdb>`), take
  their default from another place (`<to:<from>>`), or take **any number of
  arguments** (`<..>`); one bookmark can run a different command on each
  machine.
- **F4** opens a review buffer: every command and its output, in order, as
  plain text to read with the arrow keys. Alt+Up and Alt+Down jump from
  command to command.
- **`#@name`** at the end of a line works like a zsh global alias, and Tab
  completes the names.
- **Alt+Enter** reads the line as a bookmark call: `burn arch.iso`.
- **`textsmith bm export`** turns your bookmarks into bash and zsh
  functions, for any other terminal.
- **F1** opens the manual (`doc/manual.md`); **Ctrl+F1** the About window.

`textsmith --version` says which copy is running, `textsmith --keys` lists
the keys as they are set, and `textsmith --config-dir` the folder with your
bookmarks and preferences.

Site, manual and news: <https://thetextsmith.com>. The Electron edition,
`textsmith-electron`, reads the same bookmarks.

Copyright 2026 Ishe Chinyoka. The Textsmith is distributed under the GNU
General Public License, version 3 or later; see `LICENSE`.

## Install

Debian, Ubuntu, Linux Mint: the `.deb` from <https://thetextsmith.com/download/>,

    sudo apt install ./textsmith_1.0.0-1_amd64.deb

Arch: `makepkg -si` with the `PKGBUILD` in `packaging/arch`. From the
source, anywhere:

    sudo pacman -S --needed vte3 gtk3 base-devel        # Arch
    sudo apt install libvte-2.91-dev libgtk-3-dev cargo  # Debian, Ubuntu, Mint
    sh scripts/install.sh

This builds with cargo (without cargo, the prebuilt `bin/textsmith` is
used), puts `textsmith` in `~/.local/bin` and adds "The Textsmith" to the
application menu as a terminal. Earlier test copies called `textsmith`
are removed; the Electron edition is `textsmith-electron` now.

`textsmith` starts in the background, so it is safe to run from inside
itself. It takes `--working-directory DIR` and `-e command args...`, as
desktops use for "open a terminal here". To make it Super+Enter on GNOME, add
a custom shortcut that runs `textsmith`.

## Shells

Your login shell from /etc/passwd (so `chsh` applies to the next window), or
`startup.shell` in the preferences. bash and zsh are supported; anything else
runs bash. Your own `~/.bashrc`, or `.zshenv` and `.zshrc`, are read first,
then The Textsmith adds its hooks.

## Bookmarks

One per line in `bookmarks`, in The Textsmith's folder (see Preferences). Type a bookmark as a comment, with
a label ending in `: `, and press **Alt+M**:

    #vps: ssh forge                                 call it as: vps
    #sync music: rclone sync <> . --progress        sync drive:Music
    #copy: cp <src> <dst>                           copy a.txt backup/
    #burn: dd if=<iso> of=<disk:/dev/sdb> bs=1M status=progress
    #copy(2..): cp -v                               at least 2, added at the end
    #|count: wc -l                                  piped into, for #@

`<>` and `<name>` mark where arguments go, in order of first appearance; a
name used twice gets the same value. Arguments go in exactly as typed, quotes
kept. `\<>` is a literal `<>`.

**Default values.** `<name:default>` (or `<:default>` without a name) is a
place like any other, but when your arguments run out before it, the default
is filled in:

    #burn: dd if=<iso> of=<disk:/dev/sdb>
    burn arch.iso                 runs  dd if=arch.iso of=/dev/sdb
    burn arch.iso /dev/sdc        runs  dd if=arch.iso of=/dev/sdc
    burn                          "burn takes 1 to 2 arguments: iso, disk defaults to /dev/sdb; you gave 0."

Arguments fill places in order, so a default only helps when every place
after it has a default too (or a count in the label says otherwise). A
default can hold spaces and quotes, but not `>`.

**A default can be another place.** For two arguments that are usually the
same but not always:

    #send: rclone sync <from> drive:Library/<to:<from>> --progress
    send Music                    runs  rclone sync Music drive:Library/Music --progress
    send Music Songs              runs  rclone sync Music drive:Library/Songs --progress

`<to:<from>>` reads "to, which is from unless given". The default can add to
it, `<backup:<file>.bak>`. (When the two are always the same, use one name
twice: `<dir> ... <dir>`.)

**Any number of arguments.** `<..>` (or `<name..>`) takes every argument
the other places leave over, none included:

    #install: sudo apt update && sudo apt install -y <..>
    install vim git htop          runs  ... sudo apt install -y vim git htop
    #cpto: cp -v <..> <dst>
    cpto a.txt b.txt backup/      runs  cp -v a.txt b.txt backup/

There is one such place per bookmark (a name used again repeats it), and it
can have a default: `<opts..:--all-targets>`. To require at least one, give
a count: `#install(1..): sudo apt install -y <..>`. From the bookmark list,
or with Alt+Shift+M, a bookmark like this waits for its arguments on the
command line instead of running bare.

**Calling a bookmark.** Three ways, all at the shell's own prompt:

- `#@burn arch.iso` and Enter. The line can start with `#@`, or end with one:
  `ls -1F #@count` runs `ls -1F | wc -l`; `find . #@grab TODO @count` chains
  them. Bookmarks with `|` are piped into; others are simply added.
- `burn arch.iso` and **Alt+Enter**: the line is a bookmark name and its
  arguments, and can still end in `#@` additions.
- **Alt+Shift+B**: the bookmark list. Typing filters it. Enter runs a
  bookmark, or puts `#@name ` on the command line when it needs arguments; a
  piped one is added to the end of the line. Ctrl+Enter puts the bookmark on
  the line to edit, Delete removes it.

**Alt+Shift+Enter** fills a call in without running it, so you can read the
command (a dd with a default disk, say) before Enter.

The command line then shows what will actually run, and that is what goes
into your history. A mistake (a wrong name, too few arguments) is said on a
line of its own and your line stays as it was.

Tab after `#@` (or after `@` further along) completes bookmark names and says
what the bookmark takes. Elsewhere Tab is your shell's own completion,
untouched, so Orca hears only what it adds.

**Tab cycles through the choices** (in bash, as zsh does): when several
things match, the first Tab puts in the first one, each Tab after it the
next, and Shift+Tab goes back; after the last you are back to what you
typed. Each choice is said whole ("textsmith-0.6.0.zip"), not just the
part that changed. This is readline's own `menu-complete`; set
`editing.tabCompletion` to `"shell"` for bash's usual listing instead. A Tab
setup of your own in `~/.inputrc` is left alone either way.

**Alt+M** bookmarks the line (or the last command, if the line is empty), or
removes it when the same bookmark is already there. **Alt+Shift+M**
bookmarks it and runs it.

## A command per machine

When your bookmarks travel between machines (in your dotfiles, say), one
name can stand for a different command on each:

    #update: [:anvil "sudo apt update -y && sudo apt upgrade -y", :brainpower "sudo pacman -Syu --noconfirm", :hammer "sudo dnf update"]

`update` then runs the command for the machine it is on. Several names can
share a command (`:anvil :forge "..."`), `:*` is for any other machine, and
each command can have places, defaults and `<..>` of its own. Commands go in
double quotes (with `\"` for a quote inside) or single quotes. On a machine
with no command and no `:*`, the bookmark says so and runs nothing.

The machine's name, first found wins:

1. `TEXTSMITH_MACHINE`, if set.
2. `machine` in The Textsmith's folder, one line. Keep it out of your dotfiles: it is
   what makes this machine different.
3. The host name, from `/etc/hostname`. Nothing to set up.

The bookmarks file never names the machine: it travels. A bookmark that
belongs to one machine needs no list at all, `#update: sudo pacman -Syu`;
`[...]` is for one name that does different things on different machines.

## Bookmarks inside ssh, tmux and other programs

Bookmarks work at your own shell's prompt. Inside `ssh`, `tmux` or any other
program, the shell is not The Textsmith's, so `#@`, Alt+Enter and Alt+M are
not there; the review buffer still is. The bookmark list (Alt+Shift+B) is:
while a program runs, choosing a bookmark types its command into it, without
Enter, so you can hear it and then press Enter. A bookmark that takes
arguments is typed with its places (`<file>`) for you to replace.

Inside `ssh forge` (or `mosh`), a bookmark with a command per machine is one
row per machine in the list ("update, for forge: ..."), the machine you are
connected to first, so Enter types its command; Down reaches the others, and
typing a machine's name filters for it. The machine is found from the ssh
that is really running, however it was started (an alias, a bookmark), and
matched by the name you gave ssh or by the host name ssh resolves it to
(from `~/.ssh/config`). Inside tmux, this machine's row comes first.

## Taking your bookmarks to other terminals

Bookmarks are a small language of The Textsmith's own, but they need not
stay here. `textsmith bm export` writes them as ordinary bash and zsh
functions:

    textsmith bm export -o ~/.config/textsmith/bookmarks.sh

and in `~/.bashrc` or `~/.zshrc`, of any machine or terminal:

    [ -f ~/.config/textsmith/bookmarks.sh ] && . ~/.config/textsmith/bookmarks.sh

A bookmark becomes a function of the same name, which takes the same
arguments and says the same sentence when they are wrong:

    #site: rsync -rltvP --delete <source> forge:/var/www/thetextsmith/<destination>

    site() {
      (( $# == 2 )) || { printf '%s\n' 'site takes 2 arguments: source, destination; you gave '"$#." >&2; return 2; }
      rsync -rltvP --delete "${1}" forge:/var/www/thetextsmith/"${2}"
    }

Defaults, `<..>`, counts and commands per machine carry over (the function
finds the machine's name the same way); a piped bookmark (`#|count: wc -l`)
becomes a function to pipe into, `ls | count`. Bookmarks without a name, and
additions that are not commands on their own (`#quiet: 2>/dev/null`), are
left out. Run the export again after changing your bookmarks.

**Names.** A function hides any command of the same name: a builtin, a
program on your PATH, an alias or function of your own. So a bookmark whose
name is already taken is left out, and the export says so:

    install: left out; there is already a program, /usr/bin/install, which the function would hide.

Rename the bookmark, or give every function a prefix of its own with
`--prefix ts_` (`ts_install`, `ts_site`). `--keep-clashes` exports them
anyway. Functions and aliases from your own `~/.bashrc` are not checked:
make sure none shares a bookmark's name.

One difference, for the better: a quoted argument with spaces in a quoted
place (`notes 'two words'` for `#notes: find . -iname '*<>*'`) stays one
word in the function, where The Textsmith, pasting it in as text, splits it.

## The review buffer (F4)

A read-only text view of the tab's session: each command on a line of its
own (`$ make (failed, exit 2)`), its output under it. The cursor starts on the
last command.

- Up, Down and the usual text keys read it; Shift with them selects and
  Ctrl+C copies.
- Alt+Up / Alt+Down: previous / next command.
- Alt+PageUp / Alt+PageDown: previous / next command that failed.
  (Alt+Shift with the arrows are Orca's table keys.)
- Alt+End: the last command, with the newest output.
- Escape or F4: back to the terminal.

Orca reads the line the cursor lands on, as in any GTK text view. If you
also want each jump announced, set `review.speakJumps` to `true`.

The output is taken from the terminal when the command finishes, so progress
bars and `\r` rewrites appear as they ended. Full-screen programs (vim,
neomutt) are not in it, only what they left on the normal screen.

## Keys

| Key | What it does |
| --- | --- |
| F4 | Review buffer, and back |
| Alt+Enter | Run the line as a bookmark call |
| Alt+Shift+Enter | Fill the bookmark call in without running it |
| Shift+Enter | Run the line with sudo |
| Alt+M | Bookmark the line (or the last command), or remove that bookmark |
| Alt+Shift+M | Bookmark the line and run it |
| Alt+Shift+B | Bookmark list |
| Ctrl+Shift+C / Ctrl+Shift+V | Copy / paste |
| Ctrl+Shift+T / Ctrl+Shift+W | New tab / close tab |
| Ctrl+PageDown, Ctrl+Tab / Ctrl+PageUp, Ctrl+Shift+Tab | Next / previous tab |
| Ctrl+Comma | Open the preferences file |
| F1 / Ctrl+F1 | The manual / About The Textsmith |

The keys for the command line (Alt+Enter, Shift+Enter, Alt+M and so on) only
act at the shell's prompt. While a program runs, every key goes to it
unchanged, so Alt+M still reaches emacs and Shift+Enter is just Enter in
python. Everything else is readline's or zsh's own: Ctrl+R, Ctrl+B, Alt+B,
Tab.

## Preferences

`gtk-prefs.json` in The Textsmith's folder, written with every default the
first time. The folder is the first of: `TEXTSMITH_CONFIG` (or
`TEXTSMITH_HOME`); `~/.textsmith`, if it is there; `$XDG_CONFIG_HOME/textsmith`
(`~/.config/textsmith`). `textsmith --config-dir` says which.
Ctrl+Comma opens it in your text editor; it is read again when the window
gets the focus back. It is separate from the Electron version's `prefs.json`
so that neither drops the other's settings.

- `display`: `font` ("Monospace 12"), `scrollback` (lines kept),
  `colours` ("system" follows the desktop theme; "dark", "light",
  "high-contrast"), `bell`. Program colours use GNOME Terminal's palette
  on light backgrounds and a brighter one on dark ones. In the review,
  command lines are bold and blue, failed ones red; only the look changes.
- `title`: `format` at the prompt, `running` while a command runs. Both
  are `{dir}` (the directory, `~` for home), as in other terminals. Other
  fields: `{prompt}` (your PS1 without colours), `{shell}`, `{command}`,
  `{app}`.
- `startup`: `directory` new tabs start in, `shell`.
- `editing`: `tabCompletion`, `"cycle"` (Tab goes through the choices) or
  `"shell"` (bash's own listing); applies to new tabs.
- `review`: `command`, how a command line looks (`{command}`, `{status}`,
  `{dir}`, `{prompt}`), and `speakJumps`.
- `keys`: every action above. A setting may hold alternatives:
  `"Ctrl+PageDown or Ctrl+Tab"`.

## How it is put together

- `shell/textsmith.bashrc`, `shell/zsh/`: the shell integration, built into
  the program and written to `$XDG_RUNTIME_DIR/textsmith-<uid>/` when it
  starts. Marks: OSC 133 A (prompt), C (output starts), D (finished, with
  status), OSC 7 (directory), private 7772 (your prompt, for the title) and
  7773 (the line that runs, when a hook changed it), and 133 B at the end of
  your prompt, where typing starts. Enter stays the shell's own: The
  Textsmith reads the typed line from the screen, and only a line with `#@`
  goes through a hook (`\e[9989~`). A hook on every Enter would make
  readline draw the prompt again, and Orca would read it before the output.
  Tab is the same: the shell's own completion, with a hook only for a `#@`
  name (`\e[9978~`). Alt+Enter and the other keys run small hooks in
  readline or zle; the GTK side sends private keys (`\e[9978~` to
  `\e[9989~`), only at a prompt.
- `src/bookmarks.rs`: the bookmark format, ported from the Electron
  version's `bookmarks.js` with its tests, plus defaults.
- `src/helper.rs`: `textsmith bm expand | call | complete | add`, which
  the shell hooks run. It does not start GTK, so it is quick.
- `src/session.rs`: a tab. The program reads the shell's pty itself, takes
  the marks out and feeds the rest to VTE. Each mark is replaced by a query
  VTE answers at that exact point in the stream, which gives the row where a
  command's output starts and ends.
- `src/app.rs`: the window, keys, bookmark list. GTK and VTE already tell
  Orca about focus, text and the cursor; The Textsmith only speaks up
  (through ATK's notification signal) when a key would otherwise give no
  answer: "Copied", "No earlier command", a bookmark error.

## Tests

    sh scripts/manual.sh                         doc/manual.html from doc/manual.md (pandoc)
    cargo test                                   bookmarks, scanner, review, keys, export
    python3 tests/export_test.py                 exported functions behave as The Textsmith does
    python3 tests/shell_test.py bash             the integration in a real pty
    python3 tests/shell_test.py zsh
    xvfb-run -a python3 tests/gui_test.py bash   the window, typed into with xdotool
    dbus-run-session -- xvfb-run -a sh tests/a11y_run.sh   what Orca would receive
    dbus-run-session -- xvfb-run -a sh tests/a11y_run.sh tests/echo_test.py bash
                                                 Enter and Tab do not make Orca read the prompt again

## Known gaps

- There is no preferences window yet; the JSON file stands in for it.
- No errors-only view: stderr stays on the terminal, so programs see a real
  terminal there. The review buffer marks failed commands instead.
- No fish integration.
- The review buffer is a snapshot taken when it opens; Alt+End takes a new one.
- The Electron edition (`textsmith-electron`) reads the same bookmarks,
  commands per machine included.
