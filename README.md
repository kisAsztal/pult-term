# pult-term

A fork of [WezTerm](https://wezterm.org) for [pult](https://github.com/kisAsztal/pult), which gives
[Claude Code](https://claude.com/claude-code) persistent sessions and a live status on every tab. The terminal itself
is plain WezTerm; the additions are small and kept apart, so upstream can be merged:

- a status stripe on top of each tab, drawn from the tab title;
- natively animated tab titles (spinner, pulse) and smooth status colour fades, with no Lua timers;
- floating pickers (the `InputSelector`) instead of overlays that cover the whole tab;
- desktop notifications with buttons from Lua, and raising the window on click (Wayland xdg-activation).

pult-term works without pult as an ordinary WezTerm. Details, the Lua API, build and install: [PULT-TERM.md](PULT-TERM.md).

```sh
git clone https://github.com/kisAsztal/pult-term
cd pult-term
pult/install.sh            # builds with cargo and installs for the current user (Linux)
```

The rest of this file is the upstream WezTerm README.

---

# Wez's Terminal

<img height="128" alt="WezTerm Icon" src="https://raw.githubusercontent.com/wezterm/wezterm/main/assets/icon/wezterm-icon.svg" align="left"> *A GPU-accelerated cross-platform terminal emulator and multiplexer written by <a href="https://github.com/wez">@wez</a> and implemented in <a href="https://www.rust-lang.org/">Rust</a>*

User facing docs and guide at: https://wezterm.org/

![Screenshot](docs/screenshots/two.png)

*Screenshot of wezterm on macOS, running vim*

## Installation

https://wezterm.org/installation

## Getting help

This is a spare time project, so please bear with me.  There are a couple of channels for support:

* You can use the [GitHub issue tracker](https://github.com/wezterm/wezterm/issues) to see if someone else has a similar issue, or to file a new one.
* Start or join a thread in our [GitHub Discussions](https://github.com/wezterm/wezterm/discussions); if you have general
  questions or want to chat with other wezterm users, you're welcome here!
* There is a [Matrix room via Element.io](https://matrix.to/#/#wezterm:matrix.org)
  for (potentially!) real time discussions.

The GitHub Discussions and Element/Gitter rooms are better suited for questions
than bug reports, but don't be afraid to use whichever you are most comfortable
using and we'll work it out.

## Supporting the Project

If you use and like WezTerm, please consider sponsoring it: your support helps
to cover the fees required to maintain the project and to validate the time
spent working on it!

[Read more about sponsoring](https://wezterm.org/sponsor.html).

* [![Sponsor WezTerm](https://img.shields.io/github/sponsors/wez?label=Sponsor%20WezTerm&logo=github&style=for-the-badge)](https://github.com/sponsors/wez)
* [Patreon](https://patreon.com/WezFurlong)
* [Ko-Fi](https://ko-fi.com/wezfurlong)
* [Liberapay](https://liberapay.com/wez)
