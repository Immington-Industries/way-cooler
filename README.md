# Way Cooler

![Join the chat at https://gitter.im/Immington-Industries/way-cooler](https://badges.gitter.im/Immington-Industries/way-cooler.svg)
[![Crates.io](https://img.shields.io/badge/crates.io-v0.2.0-orange.svg)](https://crates.io/crate/way-cooler)
[![Build Status](https://travis-ci.org/Immington-Industries/way-cooler.svg?branch=master)](https://travis-ci.org/Immington-Industries/way-cooler)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Immington-Industries/way-cooler/)

Way Cooler is a customizeable tiling window manager written in [Rust][] for [Wayland][wayland].

# Development

Way Cooler is currently in alpha. The core features have been added and it is in a usable state, but more work is needed to make it user friendly.

## Motivation

We wanted to get experience with Rust and we found current X11 window managers to not have all the features we wanted. 

Currently there are very few fully-featured tiling window managers in the Wayland ecosystem, as most of the effort has been porting Gnome and KDE over. Although Wayland is still in early-stage development
and is not backwards compatable with existing X11 tools, we wanted to put our stake in and provide for current tiling window manager users in the future.

We take a lot of inspiration from current window managers (namely [i3][] and [awesome][]) but our goal is to exist as a unique alternative.


## Current Features
- i3-style tiling
  * Horizontal/vertical layouts
  * Nest containers with different layouts
- Client application support via an IPC
  * See an example application [here](https://github.com/Immington-Industries/Way-Cooler-Example-Clients). It displays the tree in a somewhat organized format, and is actually really helpful for both debugging the tree and understanding how subcontainers work.
  * Enables dynamic configuration at runtime, without having to reload a configuration file
  * Allows extensions of the window manager to exist as separate programs talking over the IPC
- A Lua environment designed to make extending Way Cooler simple and easy
  * Lua is the configuration format, allowing the user to enhance their window manager in any way they want.
  * Utilities library included to aid communicating with Way Cooler
- X programs supported through XWayland

## Planned Features

- i3 tabbed/stacked tiling
- Floating windows
- Tiling window through configurable Lua scripts (awesome-style)
- Server-side borders around window clients
- An [Electron](http://electron.atom.io/) powered status bar
- More customization settings

Follow the development of these features in our [issues section] or checkout our [contribution guidelines](#Contributing) if you want to help out.

# Trying out Way Cooler

If you would like to try out Way Cooler before properly installing it, then you can use the following docker command:
```bash
docker run --net=host --env="DISPLAY" --volume="$HOME/.Xauthority:/root/.Xauthority:rw" timidger/way-cooler
```

This allows you try out the window manager without having to install anything except Docker.

# Installation

You will need the following dependencies installed on your machine to install Way Cooler:
- Wayland
  * Including the server and client libraries
- wlc
  * Installation instructions can be found on [their github page](https://github.com/Cloudef/wlc)
- Weston (optional)
  * The init file defaults to using `weston-terminal` as the default terminal emulator
- Cargo
  * The package manager / build system used by Rust

Finally, to install Way Cooler simply run the following cargo command:

```shell
cargo install way-cooler
```

You can try it out while running in an X environment, or switch to a TTY and run it as a standalone

# Init File

All keyboard shortcuts (except the command to exit Way Cooler) are configurable through the init file. The recommended strategy is to copy `config/init.lua` to `$HOME/.config/way-cooler/init.lua` and edit from there. The default keybindings are:

- `Alt+Enter` Launches a terminal defined by the `way_cooler.terminal`
- `Alt+d` Opens `dmenu` to launch a program
- `Alt+p` Sends expressions to be executed directly by the Lua thread
- `Alt+Shift+Esc` Closes Way Cooler
- `Alt+v` Makes a new sub-container with a vertical layout
- `Alt+h` Makes a new sub-container with a horizontal layout
- `Alt+<arrow-key>` Switches focus to a window in that direction
- `Alt+<number-key>` Switches the current workspace
- `Alt+shift+<number-key>` Moves the focused container to another workspace

# Contributing
If you would like to contribute code, please feel free to fork and branch off of `master` and submit a pull request.

If you find bugs or have questions about the code, please [submit an issue] or ping us on gitter.

[Rust]: https://www.rust-lang.org
[wayland]: https://wayland.freedesktop.org/
[wlc]: https://github.com/Cloudef/wlc
[i3]: i3wm.org
[awesome]: https://awesomewm.org/
[issues section]: https://github.com/Immington-Industries/way-cooler/labels/features
[submit an issue]: https://github.com/Immington-Industries/way-cooler/issues/new
[gitter]: https://gitter.im/Immington-Industries/way-cooler?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge
