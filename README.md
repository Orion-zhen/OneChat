# OneChat

A minimal cross-platform LLM chat app built with Rust and [GPUI](https://gpui.rs/).

## Development

Make sure the latest stable Rust toolchain and the native dependencies required by GPUI are installed for your platform.

```sh
cargo run
```

Useful checks:

```sh
cargo fmt --check
cargo check
```

GPUI is still evolving quickly, so this project intentionally tracks the latest
published GPUI release instead of pinning a dependency version. `Cargo.lock` is
local build output and is not committed.

## Platforms

The target platforms are Linux, macOS, and Windows. Linux builds may require
X11/Wayland development libraries. macOS builds require Xcode and its command
line tools. Windows builds require the standard MSVC development tools.
