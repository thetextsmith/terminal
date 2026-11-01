#!/bin/sh
# Makes everything for the download page in dist/:
#   textsmith-<version>.tar.gz     the source, with a prebuilt bin/textsmith
#   PKGBUILD                       for Arch, with the tarball's checksum
#   textsmith_<version>-1_<arch>.deb   only on Debian, Ubuntu or Mint (needs dpkg-dev);
#                                  elsewhere, make it there later with
#                                  sh packaging/deb/build-deb.sh
#   SHA256SUMS
#   sh scripts/release.sh          (needs cargo; pandoc to remake the manual)
# Works on Arch and on Mint. The prebuilt binary is best made on the oldest
# system you support (Mint), so that it runs on the others.
set -e
here=$(cd "$(dirname "$0")/.." && pwd)
cd "$here"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
if command -v pandoc >/dev/null 2>&1; then sh scripts/manual.sh
else echo "pandoc not found: keeping doc/manual.html as it is."; fi
cargo test --release --locked
cargo build --release --locked
mkdir -p bin dist
rm -f dist/textsmith-*.tar.gz dist/PKGBUILD dist/SHA256SUMS
if command -v dpkg-shlibdeps >/dev/null 2>&1 && [ -f /etc/debian_version ]; then
  sh packaging/deb/build-deb.sh
else
  echo "Not on Debian, Ubuntu or Mint: no .deb this time (make it there with sh packaging/deb/build-deb.sh)."
fi
cp target/release/textsmith bin/textsmith
name=textsmith-$version
stage=$(mktemp -d)
mkdir "$stage/$name"
cp -r Cargo.toml Cargo.lock README.md LICENSE src shell scripts tests doc data packaging bin "$stage/$name/"
tar -C "$stage" --owner=0 --group=0 -czf "dist/$name.tar.gz" "$name"
sum=$(sha256sum "dist/$name.tar.gz" | cut -d' ' -f1)
sed "s/^pkgver=.*/pkgver=$version/; s/^sha256sums=.*/sha256sums=('$sum')/" packaging/arch/PKGBUILD > dist/PKGBUILD
(cd dist && sha256sum "$name.tar.gz" $(ls textsmith_"$version"-1_*.deb 2>/dev/null) PKGBUILD > SHA256SUMS)
echo "In dist/:"; ls -l dist
echo "To sign the sums:  gpg --local-user releases@thetextsmith.com --armor --detach-sign dist/SHA256SUMS"
