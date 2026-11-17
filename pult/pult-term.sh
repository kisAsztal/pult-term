#!/bin/sh
# Launcher for use as the default terminal (x-terminal-emulator style): `pult-term [-e command...]`
BIN="${PULT_TERM_BIN:-$HOME/.local/lib/pult-term}"
if [ "$1" = -e ] || [ "$1" = -x ]; then
  shift
  exec "$BIN/wezterm" start --class pult-term -- "$@"
fi
exec "$BIN/wezterm" start --class pult-term "$@"
