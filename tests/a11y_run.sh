#!/bin/sh
# Starts the accessibility bus, then the AT-SPI test.
#   dbus-run-session -- xvfb-run -a sh tests/a11y_run.sh
for l in /usr/libexec/at-spi-bus-launcher /usr/lib/at-spi2-core/at-spi-bus-launcher; do
  [ -x "$l" ] && { "$l" --launch-immediately & break; }
done
sleep 1
for r in /usr/libexec/at-spi2-registryd /usr/lib/at-spi2-core/at-spi2-registryd; do
  [ -x "$r" ] && { "$r" & break; }
done
sleep 1
export GTK_MODULES=gail:atk-bridge
if [ $# -gt 0 ]; then exec /usr/bin/python3.12 "$@"; fi
exec /usr/bin/python3.12 "$(dirname "$0")/a11y_test.py"
