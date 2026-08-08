# OneChat

OneChat is a personal Rust + GPUI desktop client for local, streaming conversations with OpenAI, OpenAI-compatible, Anthropic, and Gemini providers. macOS is the current validation platform.

## Prerequisites

Install the latest stable Rust toolchain, Xcode command-line tools, and Apple's Metal Toolchain:

```sh
xcode-select --install
xcodebuild -downloadComponent MetalToolchain
```

GPUI and the provider crates evolve quickly. Published dependencies intentionally use `"*"`; `gpui`/`gpui_platform` and `gpui-component`/`gpui-component-assets` track the respective upstream Git HEADs. `Cargo.lock` is local build output and is not committed, so ordinary local builds keep using the commits recorded there. Run `cargo update` (or delete the local lockfile) only when intentionally updating upstream HEADs. Breaking upstream changes are fixed directly instead of maintaining compatibility code.

## Architecture

The crate keeps reusable code independent from the GPUI desktop shell:

- `domain`: serializable conversations, catalogs, preferences, and generation contracts
- `application`: UI-independent generation preparation, reduction, cancellation, and streaming runner
- `providers`: OpenAI, Anthropic, and Gemini adapters
- `storage`: JSONC/JSON persistence behind the `Storage` facade
- `markdown`: UI-independent Markdown AST, parsing, and formula rendering
- `desktop`: GPUI application state and presentation

The desktop shell starts through `gpui_platform`, installs `gpui_component_assets`, and wraps each window in `gpui_component::Root`. Standard inputs, buttons, pickers, dialogs, forms, tabs, switches, sliders, alerts, and notifications come from `gpui-component`. `desktop/ui/theme.rs` is the single bridge from OneChat appearance settings to component theme tokens, while `desktop/ui/icons.rs` maps product semantics to component or Lucide icons. Chat layout, glass materials, product motion, Markdown/LaTeX rendering, and cross-node message selection remain OneChat-owned.

`src/main.rs` only starts `desktop::run()`. Another UI or CLI can reuse the library modules without depending on desktop internals.

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
- `Escape`: close the innermost dialog, picker, or editor; it never stops generation

## Third-party UI assets

- [`gpui-component`](https://github.com/longbridge/gpui-component) and `gpui-component-assets` are available under Apache-2.0.
- [`lucide-icons`](https://github.com/lucide-icons/lucide) glyphs are available under ISC. Lucide is derived from Feather Icons, whose original glyphs are available under MIT.

## Local data

OneChat stores plain, editable JSONC/JSON files instead of a database. App settings, providers, models, and plain-text API keys are stored in:

- macOS and Linux: `~/.config/onechat/settings.jsonc`
- Windows: `%APPDATA%\OneChat\settings.jsonc`

Reusable system prompt presets are plain Markdown files under:

- macOS and Linux: `~/.config/onechat/prompts/*.md`
- Windows: `%APPDATA%\OneChat\prompts\*.md`

The Markdown extension is for editor convenience; OneChat sends each file as plain text without parsing it.

Each conversation, including its messages and request history, has one JSON file under:

- macOS: `~/Library/Application Support/OneChat/conversations/`
- Linux: `${XDG_STATE_HOME:-~/.local/state}/onechat/conversations/`
- Windows: `%LOCALAPPDATA%\OneChat\conversations\`

The settings parser accepts JSONC comments and trailing commas. Files written by OneChat are formatted as plain JSON, which is also valid JSONC. Existing comments are not preserved when the app writes the settings file. Legacy SQLite files are neither imported nor deleted.
