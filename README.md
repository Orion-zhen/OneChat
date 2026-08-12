# OneChat

OneChat is a personal Rust + GPUI desktop client for local, streaming conversations with OpenAI, OpenAI-compatible, Anthropic, and Gemini providers. macOS is the current validation platform.

## Prerequisites

Install the latest stable Rust toolchain. On macOS, also install Xcode command-line tools and Apple's Metal Toolchain:

```sh
xcode-select --install
xcodebuild -downloadComponent MetalToolchain
```

Linux builds use the system WebKitGTK renderer for HTML/PNG conversation export. Install its development packages before building:

```sh
# Debian / Ubuntu
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev

# Arch Linux
sudo pacman -S gtk3 webkit2gtk-4.1
```

Windows PNG export uses the Microsoft Edge WebView2 Runtime, which is included with current Windows releases and can also be installed separately by Microsoft.

GPUI and the provider crates evolve quickly. Published dependencies intentionally use `"*"`; `gpui`/`gpui_platform` and `gpui-component`/`gpui-component-assets` track the respective upstream Git HEADs. `Cargo.lock` is local build output and is not committed, so ordinary local builds keep using the commits recorded there. Run `cargo update` (or delete the local lockfile) only when intentionally updating upstream HEADs. Breaking upstream changes are fixed directly instead of maintaining compatibility code.

## Architecture

The crate keeps reusable code independent from the GPUI desktop shell:

- `domain`: serializable conversations, catalogs, preferences, and generation contracts
- `application`: UI-independent attachment ingestion plus generation preparation, reduction, cancellation, and streaming execution
- `providers`: OpenAI, Anthropic, and Gemini adapters
- `storage`: JSONC/JSON persistence behind the `Storage` facade
- `mcp`: JSONC configuration and stdio/Streamable HTTP MCP server lifecycle
- `markdown`: UI-independent Markdown AST, parsing, and formula rendering
- `desktop`: GPUI coordination and feature-oriented chat, settings, inspector, and shell presentation

The desktop shell starts through `gpui_platform`, installs `gpui_component_assets`, and wraps each window in `gpui_component::Root`. Standard inputs, buttons, pickers, dialogs, forms, tabs, switches, sliders, alerts, and notifications come from `gpui-component`. `desktop/ui/theme.rs` is the semantic color center: it generates light and dark palettes from the configured theme color, feeds component theme tokens, and owns product-specific message, Markdown, status, media, glass, and selection colors. `desktop/ui/icons.rs` maps product semantics to component or Lucide icons. Chat layout, glass materials, product motion, Markdown/LaTeX rendering, and cross-node message selection remain OneChat-owned.

`src/main.rs` normally starts `desktop::run()`; on Linux and Windows the same executable can also enter a private, isolated HTML snapshot helper mode so WebKitGTK/WebView2 does not interfere with GPUI's event loop. Another UI or CLI can reuse the library modules without depending on desktop internals.

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

## Package

Install the Cargo packaging subcommand once:

```sh
cargo install cargo-packager
```

Build the release binary and package it using the current platform's native formats:

```sh
cargo package-app
```

The default outputs are `.app` and `.dmg` on macOS, an NSIS installer on Windows, and deb, AppImage, and Pacman packages on Linux. Artifacts are written to `target/release/bundle`. macOS packages use an ad-hoc signature for local installation. To create one format only, invoke `cargo packager --release --formats <format>` directly.

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

Each conversation has its own directory containing `<conversation-id>.json`, including its messages and request history:

- macOS: `~/Library/Application Support/OneChat/conversations/<conversation-id>/`
- Linux: `${XDG_STATE_HOME:-~/.local/state}/onechat/conversations/<conversation-id>/`
- Windows: `%LOCALAPPDATA%\OneChat\conversations\<conversation-id>\`

Attachment metadata and relative paths are stored in that JSON file. Attachment contents are stored under `attachments/` in the same conversation directory, so images and rendered PDF pages do not inflate the conversation log. Deleting or clearing a conversation removes its attachment files, and forking copies the attachments used by the forked history.

Hover the latest assistant response and use its export action to copy or save the visible branch as Markdown. A full archive export creates a ZIP containing that Markdown, the complete conversation JSON with all branches and requests, and the stored attachment assets.

### Attachments

Text attachments must be UTF-8 and are limited to 1 MiB. Vision models additionally accept JPEG, PNG, GIF, WebP, and PDF files, and can paste raster images directly from the clipboard into the composer. Images are limited to 10 MiB; PDFs are limited to 20 MiB and 20 pages and are rendered to one PNG image per page before being sent.

Models with the **Audio** capability accept WAV and MP3 attachments up to 10 MiB. OneChat validates the file contents rather than trusting the extension. The microphone button records a voice draft: click once to start and again to stop, press `Enter` to stop, or press `Escape` to cancel. Stopping never sends automatically. Recordings are finalized as 16 kHz mono PCM16 little-endian WAV and stop automatically at 5 minutes or 10 MiB. Audio drafts and sent audio attachments can be played, paused, and removed through their attachment cards.

Audio support differs by provider. Native OpenAI models with the **Audio** capability always use Chat Completions, including for text-only turns; other native OpenAI models continue to use Responses. OpenAI-compatible providers use Chat Completions, Gemini receives inline audio, and Anthropic does not accept audio. OneChat requests text output only and does not provide audio output or transcription as a separate local feature. Switching to a model without Audio is allowed, but generation is blocked locally while the retained request context still contains audio; audio in complete turns removed by history or context-window limits does not block the request.

Audio files and recordings use the same conversation-local attachment storage as other files. They remain local until a generation request includes them, at which point their contents are sent to the selected provider, including when replayed as retained conversation history. OneChat does not upload them to a provider file cache or retain provider file IDs. Cancelled or failed recordings are discarded without creating an attachment.

Modern Office attachments (`.docx`, `.xlsx`, and `.pptx`) are parsed locally into Markdown instead of being uploaded through a provider-specific file API. They are available with every model: the extracted Markdown is always sent, while successfully extracted embedded images are added only when the selected model supports Vision. Embedded image parsing can be disabled with **Parse Document Images** under General settings; image references are then omitted from the extracted Markdown. Office source files are limited to 20 MiB, extracted Markdown to 1 MiB, and embedded images to 20 files, 10 MiB each, and 50 MiB total.

Excel workbooks preserve the last formatted cell values saved in the file; OneChat does not recalculate formulas or expose their expressions. PowerPoint presentations provide extracted text, tables, chart cache data, notes, and individual embedded images, but slides are not rendered as full-page images, so layout, themes, SmartArt, and other visual relationships may not be preserved.

The settings parser accepts JSONC comments and trailing commas. Files written by OneChat are formatted as plain JSON, which is also valid JSONC. Existing comments are not preserved when the app writes the settings file. Legacy SQLite files are neither imported nor deleted.

## Models and conversation context

A model can store an optional **Context Window** token limit. OneChat discovers it from common provider metadata when the API exposes an unambiguous input/context limit, and the model editor allows the value to be changed or cleared. When an API does not report a limit, the value remains unknown; OneChat does not guess from the model ID.

**Conversation History** under **Settings → General → Behavior** controls how much prior context each conversation request carries. It defaults to **Unlimited**. The discrete values mean:

- **0 turns**: send no prior conversation history.
- **1–50 turns**: send at most the most recent N complete ancestor turns from the selected branch.
- **Unlimited**: apply no turn-count limit.

The current user message and system prompt do not count as history turns. The Context inspector shows the effective value directly. Changing it creates a conversation-specific override; **Reset** restores dynamic inheritance from the current global setting.

History limiting is non-destructive: it does not delete or hide local messages, attachments, branches, response candidates, or tool traces. OneChat plans context independently for each new, additional, or regenerated response and removes only whole turns, keeping each user message, selected assistant response, attachment content, and tool transcript together.

When a model has a Context Window, OneChat estimates input tokens from the resolved system prompt and serialized messages, then removes additional oldest complete turns if needed. **Unlimited** cannot bypass this window. If the system prompt and current message alone exceed it, the request fails locally with a context-length error. The estimate uses roughly one token per four text characters and 32 tokens per second of audio; encoded audio data is not counted as text. It intentionally excludes tool schemas and later agent-loop results, so provider context-length errors remain the final fallback. Models with an unknown window use only the history-turn limit.

The composer shows a compact context-capacity ring beside the send button. Its detail popover distinguishes an unknown model window from zero remaining capacity and reports whether replayed reasoning is present. After a completed request, the projection anchors itself to the provider-reported input usage from the final agent step—not cumulative tool-loop usage—and estimates only the conversation changes since that step. Reasoning counts toward the projection only when it is retained in the provider transcript for replay; display-only or hidden reasoning is not added separately.

## System prompt variables

Conversation system prompts support runtime placeholders such as `{{owner}}`. Custom variables are managed under **Settings → System Prompts** and can read fixed text, an environment variable, or the stdout of a local shell command. Only referenced variables are evaluated; use `\{{owner}}` for a literal placeholder. Command variables have a working directory, a timeout, and a 64 KiB output limit. Unknown variables and failed commands stop the request before it reaches the model.

Built-in variables include `{{onechat.date}}`, `{{onechat.datetime}}`, `{{onechat.os}}`, `{{onechat.conversation.id}}`, `{{onechat.conversation.title}}`, `{{onechat.model.name}}`, and `{{onechat.provider.name}}`. The resolved prompt and variable timings are stored with each request and shown in the Info inspector.

## Reasoning presets

Each model can expose named reasoning presets from its model editor. Known API formats are available for OpenAI Responses and Chat Completions, Anthropic adaptive effort and manual budgets, Gemini thinking levels and budgets, DeepSeek effort, and Qwen effort or budgets. The model configuration controls which presets are available and which one is used by default; a conversation can override the preset from the Model inspector, and that override is copied into each turn for regeneration.

Custom mode builds a preset from typed request parameters and `chat_template_kwargs` entries. Parameter values can be strings, integers, decimals, booleans, or null, and dotted names create nested request objects. Preset parameters override matching values from the conversation's provider-specific parameters, while OneChat-owned fields such as messages, tools, and streaming cannot be overridden. A null custom value removes an inherited provider-specific value instead of sending JSON null.

## MCP servers

OneChat reads stdio and Streamable HTTP MCP server definitions from `~/.config/onechat/mcp.jsonc`. It creates an empty file on first launch. The MCP settings UI can add, edit, delete, or import server definitions and updates the JSONC syntax tree in place so existing comments and unrelated formatting remain intact. Servers use commands already installed on the system; OneChat does not install Node or Python runtimes and does not interpret commands as shell expressions. Windows may use `cmd.exe` internally when launching `.cmd` or `.bat` wrappers such as `npx.cmd`.

```jsonc
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": [
        "-y",
        "@modelcontextprotocol/server-filesystem",
        "/Users/orion/repo"
      ]
    },
    "fetch": {
      "command": "uv",
      "args": ["tool", "run", "mcp-server-fetch"]
    },
    "exa": {
      "url": "https://mcp.exa.ai/mcp",
      "headers": { "X-API-Key": "plain-text-value" },
      "proxy": "socks5://127.0.0.1:1080",
      "disabledTools": ["web_fetch_exa"]
    },
    "secure-api": {
      "url": "https://example.com/mcp",
      "bearerToken": "plain-text-token"
    },
    "oauth-api": {
      "url": "https://example.com/mcp",
      "oauth": {
        "flow": "authorizationCode",
        "clientId": "optional-pre-registered-client",
        "scopes": ["read", "write"],
        "callbackPort": 0
      }
    },
    "machine-api": {
      "url": "https://example.com/mcp",
      "oauth": {
        "flow": "clientCredentials",
        "clientId": "client-id",
        "clientSecret": "client-secret",
        "scopes": ["tools"]
      }
    },
    "local-python": {
      "enabled": false,
      "command": "uv",
      "args": ["run", "python", "server.py"],
      "cwd": "/absolute/path/to/project",
      "env": {
        "API_TOKEN": "plain-text-value"
      }
    }
  }
}
```

`enabled` defaults to `true`; `args`, `env`, `headers`, and `disabledTools` default to empty values. Server and tool switches in the MCP settings page update `enabled` and `disabledTools` directly. `cwd`, when present, must be absolute. A relative command containing a path separator is resolved from `cwd` and therefore requires it; an absolute command is always used directly. Arguments and environment values are literal: `~`, `$HOME`, and shell syntax are not expanded.

For bare commands such as `npx` or `uv`, OneChat reconstructs the user's execution PATH instead of relying only on the environment inherited by a GUI launcher. macOS and Linux read PATH from the user's login shell with a timeout, then fall back to the inherited PATH and standard system/user binary directories. Windows refreshes the current user's environment block and honors `PATHEXT`, including `.exe`, `.cmd`, and `.bat` wrappers. The resulting PATH is also passed to the MCP child process so interpreter-based launchers can find dependencies such as `node`. A server's explicit `env.PATH` overrides automatic discovery. Absolute paths remain the most deterministic option.

Native macOS App Bundles, Windows desktop installers, AppImage, deb, and rpm packages can start host executables directly. Sandboxed distributions such as macOS App Sandbox, Windows AppContainer, strict Snap, and Flatpak require a separate host-execution integration and are not supported for local stdio MCP servers.

HTTP servers support custom `headers`, HTTP/SOCKS `proxy`, `bearerToken`, interactive authorization-code OAuth with PKCE, and OAuth client credentials. `bearerToken`, `oauth`, and an explicit `Authorization` header are mutually exclusive. Client credentials require both `clientId` and `clientSecret`. Interactive OAuth can omit `clientId` when the authorization server supports dynamic client registration; use the key action on the server card to open browser authorization. Access and refresh tokens are cached in `mcp-oauth/` next to `mcp.jsonc` with owner-only file permissions and are invalidated when the server URL or OAuth configuration changes. Configured secrets remain plain text in `mcp.jsonc`.

The MCP Servers settings page shows resolved executables, connection failures, and discovered tools. Server cards are collapsed by default and can run an isolated connection test. Servers can be configured field by field, or imported by pasting a JSON/JSONC object containing `mcpServers`; imports merge by server ID and replace matching definitions. Open Config and Reload actions remain available.

OneChat exposes discovered MCP tools to models with the Tools capability and automatically runs tool-call loops. New OpenAI, Anthropic, and Gemini models enable Tools by default; OpenAI-compatible models require enabling it in the model editor. Each conversation initially follows the global tool defaults, but the Tools inspector can override each tool—including enabling a globally disabled tool—without changing the global configuration. A disabled MCP server remains unavailable. Forks inherit the conversation selection, and the controls are locked during an active generation. Each assistant response shows live, expandable tool cards with arguments, results, errors, and durations; these traces are persisted, copied when a conversation is forked, cleared on regeneration, and marked interrupted after an unexpected restart. The request inspector reports cumulative tool count and execution time.

Tool results currently support text, structured JSON, and embedded text resources. Calls run automatically and may run in parallel, with a maximum of eight model steps and 256 KiB per tool result. Direct resource browsing, prompts, sampling, and per-call approval are not supported.
