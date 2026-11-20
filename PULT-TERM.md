# pult-term

pult-term is [WezTerm](https://wezterm.org) with a few additions for [pult](https://github.com/kisAsztal/pult). Upstream
documentation applies to everything else. The fork follows upstream `main` (see [Merging upstream](#merging-upstream)).

## What is added

| Feature | Where | Notes |
|---------|-------|-------|
| Status stripe on tabs | `crates/wezterm-gui/src/termwindow/render/fancy_tab_bar.rs` (`pult_accent`) | A tab title starting with `▎` is the stripe: the fork removes the character and draws a 3 px stripe in its colour on top of a square tab. Stock WezTerm shows the same title as a left stripe. |
| Native tab animation | same file (`pult_animate`) | A braille character in a title is replaced by spinner frames (10/s). A title `A \u{e000} B` pulses: A and B are the same label in the colours of the two ends of the pulse (sine, 1.6 s, ~30 frames/s). Runs only while such a tab exists, also in an unfocused window (half as often). `window.pult_tab_anim` tells Lua that it is available. |
| Status colour fade | same file (`pult_fade`) | The tab background and stripe fade into the new colour over ~220 ms. A title containing `\u{e001}` flashes once on change (done / failed). |
| Floating pickers | `crates/wezterm-gui/src/termwindow/pultselector.rs` | Every `InputSelector` with an `action_callback` becomes a floating panel under the tab bar. Same event back to Lua. Typing filters; ↑↓ / Ctrl+P/N, PgUp/PgDn, Enter, Esc; in a non-fuzzy list 1–9 select. Ctrl+D / Delete on an entry whose id starts with `rm:` removes the row, keeps the list open and sends the id `remove:<id>`. Colours: `command_palette_bg_color` / `_fg_color`. No mouse support yet. |
| Notifications with buttons | `crates/wezterm-toast-notification/src/pult.rs`, `crates/wezterm-gui/src/termwindow/pult.rs` | Linux (D-Bus) only; macOS and Windows get a plain notification. |
| Raise on click (Wayland) | `crates/window/src/os/wayland/` | `WindowOps::activate_with_token` with xdg-activation. GNOME 46 does not send the token; the window is then focused plainly. |
| Update check off by default | `crates/config/src/config.rs` | The upstream check looks at the WezTerm releases. |
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

This repository does not carry the upstream history: it starts with an `Import WezTerm at wez/wezterm@…` commit, and
each upstream update is one squashed `Update WezTerm to wez/wezterm@…` commit, whose message holds the full upstream
commit id. A temporary, local-only graft gives git the common ancestor for the merge:

```sh
git remote add upstream https://github.com/wez/wezterm.git    # once
git fetch upstream
last=$(git log -1 --format=%H -E --grep='^(Import|Update) WezTerm (at|to) wez/wezterm@')
base=$(git log -1 --format=%B "$last" | grep -oE '[0-9a-f]{40}' | head -1)
git replace --graft "$last" $(git rev-parse -q --verify "$last^") "$base"   # local only
git merge --squash -X find-renames upstream/main
# resolve the conflicts, then:
git commit -m "Update WezTerm to wez/wezterm@$(git rev-parse --short=12 upstream/main)" \
           -m "Upstream commit $(git rev-parse upstream/main)."
git replace -d "$last"                                         # before pushing
```

The fork keeps the workspace crates in `crates/` (upstream has them at the top level), and it has dropped upstream's
`.github`, `docs`, `ci`, `nix` and packaging files. Git follows the renames, so most upstream changes merge as they are;
files that upstream changes and the fork deleted come back as modify/delete conflicts: delete them again. Conflicts are most likely in `fancy_tab_bar.rs`, `paint.rs` and the
Wayland window code.

The tab bar font needs an explicit fallback for braille glyphs (the pult config uses DejaVu Sans / Apple Braille), otherwise
every redraw walks the system fallback fonts (~10 ms).
