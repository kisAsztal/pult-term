#!/usr/bin/env bash
# Install pult-term for the current user (Linux): release build, binaries in ~/.local/lib/pult-term, icon,
# .desktop entry (window class: pult-term) and the `pult-term` launcher. Idempotent. A running instance is not
# stopped; the new binary is used the next time pult-term starts.
#
# Desktop integration (GNOME) is opt-in:
#   --set-default   make pult-term the default terminal (gsettings, xdg-terminals.list, a custom terminal shortcut)
#   --dock          add pult-term to the dock favourites
#   --blur          add pult-term to the Blur my Shell whitelist
# Other options:
#   --no-build      install target/release as it is (e.g. built on another machine)
#   --uninstall     remove what this script installed (the desktop integration is not reverted)
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN=$HOME/.local/lib/pult-term
cd "$ROOT"

build=1 set_default=0 dock=0 blur=0 uninstall=0
for arg in "$@"; do
  case "$arg" in
    --no-build) build=0 ;;
    --set-default) set_default=1 ;;
    --dock) dock=1 ;;
    --blur) blur=1 ;;
    --uninstall) uninstall=1 ;;
    -h|--help) sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $arg (see --help)" >&2; exit 2 ;;
  esac
done

if [ "$uninstall" = 1 ]; then
  rm -rf "$BIN"
  rm -f ~/.local/bin/pult-term ~/.local/share/applications/pult-term.desktop \
        ~/.local/share/icons/hicolor/scalable/apps/pult-term.svg
  update-desktop-database -q ~/.local/share/applications 2>/dev/null || true
  echo "removed. If pult-term was your default terminal, change it back in Settings."
  exit 0
fi

# no wezterm-mux-server: the sessions live in pultd
[ "$build" = 0 ] || cargo build --release -p wezterm-gui -p wezterm

mkdir -p "$BIN" ~/.local/bin ~/.local/share/applications ~/.local/share/icons/hicolor/scalable/apps
for b in wezterm wezterm-gui; do
  # copy and rename, so a running instance keeps its old binary
  install -m755 "target/release/$b" "$BIN/$b.new" && mv -f "$BIN/$b.new" "$BIN/$b"
done
install -m755 pult/pult-term.sh ~/.local/bin/pult-term
install -m644 pult/pult-term.svg ~/.local/share/icons/hicolor/scalable/apps/pult-term.svg
sed "s#@BIN@#$BIN#g" pult/pult-term.desktop > ~/.local/share/applications/pult-term.desktop
update-desktop-database -q ~/.local/share/applications 2>/dev/null || true
gtk-update-icon-cache -q -t ~/.local/share/icons/hicolor 2>/dev/null || true

if [ "$dock" = 1 ] && command -v gsettings >/dev/null; then
  python3 - <<'PY'
import ast, subprocess
key = ('org.gnome.shell', 'favorite-apps')
apps = ast.literal_eval(subprocess.run(['gsettings', 'get', *key], capture_output=True, text=True).stdout.strip().removeprefix('@as '))
if 'pult-term.desktop' not in apps:
    subprocess.run(['gsettings', 'set', *key, str(apps + ['pult-term.desktop'])], check=False)
PY
fi

if [ "$set_default" = 1 ]; then
  if command -v gsettings >/dev/null; then
    # GNOME 47+ has no built-in terminal shortcut: a custom shortcut that starts another terminal now starts pult-term
    python3 - <<'PY'
import ast, os, subprocess
def get(schema_key):
    out = subprocess.run(['gsettings', 'get', *schema_key], capture_output=True, text=True).stdout.strip()
    return ast.literal_eval(out.removeprefix('@as '))
mk = 'org.gnome.settings-daemon.plugins.media-keys'
for path in get((mk, 'custom-keybindings')):
    key = (f'{mk}.custom-keybinding:{path}', 'command')
    cmd = get(key).split()
    if cmd and os.path.basename(cmd[0]) in ('gnome-terminal', 'x-terminal-emulator', 'kgx', 'gnome-console', 'ptyxis'):
        subprocess.run(['gsettings', 'set', *key, os.path.expanduser('~/.local/bin/pult-term')], check=False)
PY
    gsettings set org.gnome.desktop.default-applications.terminal exec "$HOME/.local/bin/pult-term"
    gsettings set org.gnome.desktop.default-applications.terminal exec-arg -e
  fi
  # xdg-terminal-exec / GLib (Terminal=true apps): pult-term first
  list=~/.config/xdg-terminals.list
  mkdir -p "$(dirname "$list")"
  { echo pult-term.desktop; grep -vx pult-term.desktop "$list" 2>/dev/null || true; } > "$list.new" && mv "$list.new" "$list"
fi

if [ "$blur" = 1 ] && command -v dconf >/dev/null; then
  key=/org/gnome/shell/extensions/blur-my-shell/applications/whitelist
  cur=$(dconf read $key || true)
  if [ -n "$cur" ] && ! grep -q "'pult-term'" <<<"$cur"; then
    dconf write $key "$(sed "s/]\$/, 'pult-term']/; s/\[, /[/" <<<"$cur")"
  fi
fi

echo "installed: $BIN ($("$BIN/wezterm" --version))"
