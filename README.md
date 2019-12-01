# <img src="http://i.imgur.com/OGeL1nN.png" width="60"> Way Cooler
Way Cooler is a plugin-based compositor.

## Building

To build Way Cooler, ensure you have meson installed (as well as [wlroots][], or
use the `subprojects/` directory and build it locally).

Then, execute:

```bash
meson build
ninja -C build
```

To run the compositor simply execute `build/way-cooler/way-cooler` in a TTY or
any existing window manager.

## Plugins

Though technically it can run standalone, the compositor is really not usable by
itself. It can not spawn programs or more/resize clients without client side
borders. All other traditional functionality like backgrounds, server side
decorations, and status bars are also missing. In order to get this
functionality back plugins must be used.

### Awesome (incomplete)

There is an [incomplete port of awesome](https://github.com/way-cooler/awesome)
that I do not plan to finish. Those changes should be a good enough basis to
implement Awesome against Way Cooler.

## Development

Way Cooler is under active development. If you would like to contribute you can
contact me best on [the sway-devel IRC](https://webchat.freenode.net/#sway-devel).

**Master is not usable for production**. There are old versions of Way Cooler
that do work, however:

* Is written in Rust and must be built with `cargo`.
* They use an old framework, [wlc][], and thus are very limited and buggy.
* Was not designed to be plugin based, but instead has [i3][] tiling and its own
  (very incomplete) Lua libraries based loosely on [AwesomeWM][].

[Wayland]: https://wayland.freedesktop.org/
[wlc]: https://github.com/Cloudef/wlc
[AwesomeWM]: https://awesomewm.org/
[wlroots]: https://github.com/swaywm/wlroots
[i3]: https://i3wm.org
