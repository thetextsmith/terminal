#!/bin/sh
# Builds The Textsmith and puts it on your PATH and in your desktop's
# application menu.
#   sh scripts/install.sh
set -e
here=$(cd "$(dirname "$0")/.." && pwd)
bin="$HOME/.local/bin"
apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
mkdir -p "$bin" "$apps"
if command -v cargo >/dev/null 2>&1; then
  (cd "$here" && cargo build --release)
  src="$here/target/release/textsmith"
elif [ -x "$here/bin/textsmith" ]; then
  echo "cargo not found: installing the prebuilt bin/textsmith"
  src="$here/bin/textsmith"
else
  echo "Neither cargo nor a prebuilt binary found." >&2; exit 1
fi
# The Electron edition used to be called textsmith; it is textsmith-electron now.
if [ -L "$bin/textsmith" ]; then
  echo "Replacing $bin/textsmith, a link to $(readlink "$bin/textsmith") (the Electron edition is textsmith-electron now)."
  rm -f "$bin/textsmith"
fi
install -m 755 "$src" "$bin/textsmith"
# Earlier test versions were called textsmith-gtk.
rm -f "$bin/textsmith-gtk" "$apps/textsmith-gtk.desktop"
sed "s|^Exec=textsmith|Exec=$bin/textsmith|" "$here/data/textsmith.desktop" > "$apps/textsmith.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" >/dev/null 2>&1 || true
echo "Installed: $("$bin/textsmith" --version)"
echo "Menu entry: $apps/textsmith.desktop"
case ":$PATH:" in *":$bin:"*) ;; *) echo "Note: $bin is not on your PATH yet; add it in ~/.bashrc to type 'textsmith'.";; esac
