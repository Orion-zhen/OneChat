# OneChat

OneChat is a personal Rust + GPUI desktop client for local, streaming conversations with OpenAI, OpenAI-compatible, Anthropic, and Gemini providers. macOS is the current validation platform.

## Prerequisites

Install the latest stable Rust toolchain, Xcode command-line tools, and Apple's Metal Toolchain:

```sh
xcode-select --install
xcodebuild -downloadComponent MetalToolchain
```

GPUI and the provider crates evolve quickly. All direct dependencies intentionally track their latest published versions with `"*"`; `Cargo.lock` is local build output and is not committed. Breaking upstream changes are fixed directly instead of maintaining compatibility code.

## Run and verify

```sh
cargo run
```

Useful checks:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo check
```

The primary shortcuts use Command on macOS and Ctrl on Linux/Windows:

- `Cmd/Ctrl+N`: new conversation
- `Cmd/Ctrl+K`: command palette
- `Cmd/Ctrl+L`: model picker
- `Cmd/Ctrl+Shift+S`: toggle sidebar
- `Cmd/Ctrl+,`: settings
- `Enter`: send or confirm; `Shift+Enter`: insert a newline
- `Escape`: close the active overlay or editor; it never stops generation

## Local data

The SQLite database is stored at:

```text
~/Library/Application Support/OneChat/onechat.sqlite3
```

Provider API keys are intentionally stored as plain text in this database. OneChat has no schema migration layer; after an incompatible development change, quit the app and reset it with:

```sh
rm -f "$HOME/Library/Application Support/OneChat/onechat.sqlite3"*
```
