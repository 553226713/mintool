# Adabraka GPUI Linux build fix

`adabraka-gpui/` is the published `adabraka-gpui 0.5.1` crate, with its original license files retained. Upstream source commit recorded in the crate: `79d73db78d1fea507e707c7c4284276413c3ac54`.

The X11 compile fix adds one trait import in `src/platform/linux/global_hotkey.rs`:

```rust
use x11rb::connection::Connection as _;
```

The published X11 implementation calls `XCBConnection::setup()` without bringing the `Connection` trait into scope, so it cannot compile on Linux with X11 enabled. This patch preserves both X11 and Wayland support. The published Linux renderer also has host/shader layout mismatches: `Quad` is 248 bytes on the host but 256 bytes in WGSL; the `Shadow` host stride is 76 bytes but WGSL requires 80. `src/scene.rs` now applies Linux-only eight-byte alignment to `TransformationMatrix` and `Shadow`. This puts `Quad.transform` at byte 224, `Quad.blend_mode` at byte 248, and produces the correct 256/80-byte array strides. Compile-time size/offset assertions protect these layouts; the renderer's startup checks validate all shader structures.

The X11 close handler also releases its client-state borrow before invoking window close callbacks. Those callbacks may re-enter the client through `App::quit`; holding the borrow causes a `RefCell already borrowed` panic when closing the last window.

The fixes are limited to Linux; other platform ABIs are unchanged. Remove the Cargo patch when a published upstream version includes these fixes.

`Cargo.lock` also pins `libc` to `0.2.186`: the fork's transitive `xattr 0.2.3` dependency references the Linux `ENOATTR` alias removed in newer `libc` releases. Use `--locked` for reproducible builds.

Upstream vendored files retain their own formatting and file layout; application source conventions apply to `src/`.

## X11 Chinese input method decoding

`xim-ctext/` is the published `xim-ctext 0.3.0` crate (source commit
`65f77e44524ba487285a63aac4080faf2c05b4ed`, subdirectory `xim-ctext`).
Its decoder rejected GB2312 Compound Text, which IBus/libpinyin sends while
composing Chinese. It now decodes GB2312, JIS X0208 and KS C5601 in GL/GR,
including character-set switches, ASCII and UTF-8 segments within one message.
Malformed/truncated data returns an error rather than slicing past a buffer.
The root `tests/ime.rs` contains real Xlib output and malformed-message regressions.
Its MIT license is retained from the source repository.

`zed-xim/` is the published `zed-xim 0.4.0-zed` crate (source commit
`471c10cdf97b3fcc2b7e0b3060f0085d1a7a27ec`), with its MIT license retained.
Three Compound Text decode calls in `src/client.rs` used `expect`, making an
unsupported or malformed input method message terminate the entire app. They
now log and return `ClientError::InvalidReply`; GPUI handles that error without
terminating the event loop. Neither patch disables Chinese input or the IME.

## Dock component and native terminal embedding

`gpui-component/` and `gpui-component-assets/` are the published 0.5.1
crates from https://github.com/longbridge/gpui-component, with their
Apache-2.0 license retained. Dependencies are bound to the existing Adabraka
GPUI fork so panel entities share one type system and event loop. Local changes
are listed in [gpui-component/MINTOOL-PATCHES.md](gpui-component/MINTOOL-PATCHES.md).
The application's terminal/file content implements the library's `Panel` trait;
tab drag/drop, edge docking, group splitting and divider dragging remain library
behavior. The initial center always has a split parent, as required by the
library's `TabPanel::split_panel` implementation.

`adabraka-gpui/src/platform/linux/x11/window.rs` now implements the public
`HasWindowHandle` and `HasDisplayHandle` methods, which were upstream
`unimplemented!()` stubs. They borrow the existing live XCB window and connection
instead of creating new handles. This is required for hosting an external
terminal's child window. Wayland retains its existing handles and the application
falls back to the built-in terminal in automatic mode.
