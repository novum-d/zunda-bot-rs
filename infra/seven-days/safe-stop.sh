#!/usr/bin/env bash
set -euo pipefail
TELNET_HOST=${TELNET_HOST:-127.0.0.1}
TELNET_PORT=${TELNET_PORT:-8081}
TELNET_PASSWORD_FILE=${TELNET_PASSWORD_FILE:-/srv/seven-days-data/telnet-password}
if [ -r "$TELNET_PASSWORD_FILE" ]; then
  PASSWORD=$(<"$TELNET_PASSWORD_FILE")
  { printf '%s\n' "$PASSWORD"; sleep 1; printf 'say Server shutting down in 30 seconds\nsaveworld\n'; sleep 30; printf 'shutdown\n'; } | telnet "$TELNET_HOST" "$TELNET_PORT" >/dev/null
fi
for _ in $(seq 1 60); do pgrep -f '7DaysToDieServer' >/dev/null || exit 0; sleep 2; done
exit 1
