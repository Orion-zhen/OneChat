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

OneChat stores plain, editable JSONC/JSON files instead of a database. App settings, providers, models, and plain-text API keys are stored in:

- macOS and Linux: `~/.config/onechat/settings.jsonc`
- Windows: `%APPDATA%\OneChat\settings.jsonc`

Each conversation, including its messages and request history, has one JSON file under:

- macOS: `~/Library/Application Support/OneChat/conversations/`
- Linux: `${XDG_STATE_HOME:-~/.local/state}/onechat/conversations/`
- Windows: `%LOCALAPPDATA%\OneChat\conversations\`

The settings parser accepts JSONC comments and trailing commas. Files written by OneChat are formatted as plain JSON, which is also valid JSONC. Existing comments are not preserved when the app writes the settings file. Legacy SQLite files are neither imported nor deleted.
