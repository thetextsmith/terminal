#!/bin/sh
# Makes doc/manual.html, the page F1 opens, from doc/manual.md.
#   sh scripts/manual.sh          (needs pandoc)
set -e
here=$(cd "$(dirname "$0")/.." && pwd)
pandoc "$here/doc/manual.md" -f markdown -t html5 --standalone --toc --toc-depth=2 \
  --metadata lang=en --css=/dev/null --include-in-header="$here/doc/manual.css.html" \
  -o "$here/doc/manual.html"
sed -i 's|<link rel="stylesheet" href="/dev/null" />||' "$here/doc/manual.html"
echo "Wrote $here/doc/manual.html"
