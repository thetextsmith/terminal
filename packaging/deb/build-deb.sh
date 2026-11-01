#!/bin/sh
# Builds textsmith_<version>-1_<arch>.deb from a release build.
#   sh packaging/deb/build-deb.sh            (run from the source folder; needs cargo, dpkg-dev)
set -e
here=$(cd "$(dirname "$0")/../.." && pwd)
cd "$here"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
arch=$(dpkg --print-architecture)
cargo build --release --locked
root=$(mktemp -d)/textsmith_${version}-1_${arch}
mkdir -p "$root/DEBIAN" "$root/usr/bin" "$root/usr/share/applications" "$root/usr/share/doc/textsmith"
install -m 755 target/release/textsmith "$root/usr/bin/textsmith"
install -m 644 data/textsmith.desktop "$root/usr/share/applications/textsmith.desktop"
install -m 644 README.md doc/manual.html "$root/usr/share/doc/textsmith/"
cat > "$root/usr/share/doc/textsmith/copyright" <<COPY
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: textsmith
Source: https://thetextsmith.com

Files: *
Copyright: 2026 Ishe Chinyoka
License: GPL-3+
 On Debian systems, the full text of the GNU General Public License
 version 3 can be found in /usr/share/common-licenses/GPL-3.
COPY
# The libraries the binary needs, as package dependencies.
work=$(mktemp -d); mkdir -p "$work/debian"
printf 'Source: textsmith\n\nPackage: textsmith\nArchitecture: any\n' > "$work/debian/control"
deps=$(cd "$work" && dpkg-shlibdeps -O "$root/usr/bin/textsmith" 2>/dev/null | sed -n 's/^shlibs:Depends=//p')
size=$(du -sk "$root/usr" | cut -f1)
cat > "$root/DEBIAN/control" <<CONTROL
Package: textsmith
Version: ${version}-1
Section: x11
Priority: optional
Architecture: ${arch}
Depends: ${deps}
Installed-Size: ${size}
Maintainer: Ishe Chinyoka <releases@thetextsmith.com>
Homepage: https://thetextsmith.com
Provides: x-terminal-emulator
Description: terminal for anyone who works with text, commands and the shell
 The Textsmith is a GTK and VTE terminal written for screen readers first.
 It adds bookmarks (commands saved once, with places for their arguments,
 default values and a command per machine, run by name), a review buffer
 of every command and its output (F4), and bookmarks exported as shell
 functions for other terminals.
CONTROL
cat > "$root/DEBIAN/postinst" <<'POST'
#!/bin/sh
set -e
if [ "$1" = configure ] && command -v update-alternatives >/dev/null; then
  update-alternatives --install /usr/bin/x-terminal-emulator x-terminal-emulator /usr/bin/textsmith 30
fi
POST
cat > "$root/DEBIAN/prerm" <<'PRE'
#!/bin/sh
set -e
if [ "$1" = remove ] && command -v update-alternatives >/dev/null; then
  update-alternatives --remove x-terminal-emulator /usr/bin/textsmith
fi
PRE
chmod 755 "$root/DEBIAN/postinst" "$root/DEBIAN/prerm"
mkdir -p dist
dpkg-deb --root-owner-group --build "$root" "dist/textsmith_${version}-1_${arch}.deb"
echo "Wrote dist/textsmith_${version}-1_${arch}.deb"
