# CLAUDE.md

pult-term: a WezTerm fork for [pult](https://github.com/kisAsztal/pult). Read `PULT-TERM.md` for what is added and where.

- Keep changes to upstream code small and well separated, so upstream updates stay easy. New code goes into
  new `pult*.rs` files; mark edits inside upstream functions with a `pult:` comment.
- The upstream history is not in this repository: update from upstream only as `PULT-TERM.md` ("Merging upstream")
  describes, as one squashed commit. Never `git merge upstream/main` directly, and never push a `refs/replace` ref:
  either would bring the whole WezTerm history into the repository.
- Build: `cargo build --release -p wezterm-gui -p wezterm`. Install: `pult/install.sh`.
- Do not start a GUI from a session that has `CLAUDECODE` / `CLAUDE_CODE_*` set without unsetting them first.
- The crates live in `crates/` (not at the top level, as upstream has them). Upstream's `.github`, `docs`, `ci`, `nix` stay removed.
- No machine names, addresses or secrets anywhere in the repository.
