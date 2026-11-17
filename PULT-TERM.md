# pult-term

pult-term is [WezTerm](https://wezterm.org) with a few additions for [pult](https://github.com/kisAsztal/pult). Upstream
documentation applies to everything else. The fork follows upstream `main`: `git fetch upstream && git merge upstream/main`.

## What is added

| Feature | Where | Notes |
|---------|-------|-------|
| Status stripe on tabs | `wezterm-gui/src/termwindow/render/fancy_tab_bar.rs` (`pult_accent`) | A tab title starting with `▎` is the stripe: the fork removes the character and draws a 3 px stripe in its colour on top of a square tab. Stock WezTerm shows the same title as a left stripe. |
| Native tab animation | same file (`pult_animate`) | A braille character in a title is replaced by spinner frames (10/s). A title `A \u{e000} B` pulses: A and B are the same label in the colours of the two ends of the pulse (sine, 1.6 s, ~30 frames/s). Runs only while such a tab exists, also in an unfocused window (half as often). `window.pult_tab_anim` tells Lua that it is available. |
| Status colour fade | same file (`pult_fade`) | The tab background and stripe fade into the new colour over ~220 ms. A title containing `\u{e001}` flashes once on change (done / failed). |
| Floating pickers | `wezterm-gui/src/termwindow/pultselector.rs` | Every `InputSelector` with an `action_callback` becomes a floating panel under the tab bar. Same event back to Lua. Typing filters; ↑↓ / Ctrl+P/N, PgUp/PgDn, Enter, Esc; in a non-fuzzy list 1–9 select. Colours: `command_palette_bg_color` / `_fg_color`. No mouse support yet. |
| Notifications with buttons | `wezterm-toast-notification/src/pult.rs`, `wezterm-gui/src/termwindow/pult.rs` | Linux (D-Bus) only; macOS and Windows get a plain notification. |
| Raise on click (Wayland) | `window/src/os/wayland/` | `WindowOps::activate_with_token` with xdg-activation. GNOME 46 does not send the token; the window is then focused plainly. |
| Update check off by default | `config/src/config.rs` | The upstream check looks at the WezTerm releases. |
| Release profile | `Cargo.toml` | `strip` + thin LTO: a smaller binary, a somewhat longer build. |

Changes to upstream code are kept small, and marked with a `pult:` comment where they sit inside upstream functions.

## Lua API

```lua
window:pult_notify {
  tag = 'pane-12',                      -- one notification per tag; a new one replaces the old
  title = 'devbox:app · done',
  message = 'your turn',
  actions = { { 'default', '' }, { 'open', 'Open' }, { 'bg-allow', 'Allow' } },
  timeout = 0,                          -- ms, 0 = until closed
}
window:pult_notify_close('pane-12')

wezterm.on('pult-notification-action', function(window, tag, action) end)
```

`default` is a click on the notification body. An action id starting with `bg-` does not raise the window. `window.pult_tab_anim`
is `true` in this fork and absent in stock WezTerm, so a config can tell them apart (the pult config does).

## Build and install

```sh
cargo build --release -p wezterm-gui -p wezterm     # the mux server is not needed: sessions live in pultd
pult/install.sh                                      # installs to ~/.local/lib/pult-term, icon, launcher, .desktop
pult/install.sh --set-default --dock --blur          # optional GNOME integration (see --help)
pult/install.sh --no-build                           # install an existing target/release
pult/install.sh --uninstall
```

Linux build dependencies are those of WezTerm: see `get-deps`. macOS and Windows are not built or tested by this fork.

To try it without disturbing running windows, start a separate instance with its own window class:
`target/release/wezterm start --always-new-process --class pult-restore-test`.

## Merging upstream

```sh
git fetch upstream && git merge upstream/main
```

Upstream's `.github` directory is removed in this fork (its workflows are heavy and its templates point to WezTerm
channels). If a merge brings it back, delete it again. Conflicts are most likely in `fancy_tab_bar.rs`, `paint.rs` and the
Wayland window code.

The tab bar font needs an explicit fallback for braille glyphs (the pult config uses DejaVu Sans / Apple Braille), otherwise
every redraw walks the system fallback fonts (~10 ms).
