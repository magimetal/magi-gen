<p align="center">
  <h1 align="center">magi-gen</h1>
</p>

<p align="center">
  Standalone Rust CLI for generating images from text prompts using Codex/ChatGPT subscription OAuth or any OpenAI-compatible Responses API. Supports transparent background output via automatic chromakey removal.
</p>

<p align="center">
  <img alt="Rust 2024" src="https://img.shields.io/badge/Rust-2024-000000?logo=rust&logoColor=white">
  <img alt="Version 0.0.1" src="https://img.shields.io/badge/version-0.0.1-blue">
  <img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-green">
  <img alt="Binary: magi-gen" src="https://img.shields.io/badge/binary-magi--gen-4EAA25">
  <a href="https://crates.io/crates/magi-gen"><img alt="crates.io" src="https://img.shields.io/badge/crates.io-magi--gen-orange"></a>
</p>

> **TL;DR:** `cargo install magi-gen`, run `magi-gen login codex`, then `magi-gen "a cat in a spacesuit" -o cat.png`.

---

## Table of contents

- [What it does](#what-it-does)
- [Quick start](#quick-start)
- [Transparent backgrounds](#transparent-backgrounds)
- [Commands](#commands)
- [Options](#options)
- [Files and configuration](#files-and-configuration)
- [Security](#security)
- [License](#license)

## What it does

| Capability | Details |
| --- | --- |
| **Two providers** | **Codex** (default): ChatGPT subscription OAuth via PKCE login with automatic token refresh. **OpenAI-compatible**: API key + base URL against any Responses API endpoint. |
| **Image generation** | Uses OpenAI Responses API with hosted `image_generation` tool. Streams via SSE, captures partial and final images. |
| **Transparent output** | `--transparent` flag injects a solid pink background prompt, then removes it post-generation via corner-color detection and `rustychroma` chromakey. Outputs RGBA PNG with alpha channel. |
| **Output formats** | PNG (default), WebP (~30% smaller), or JPEG via `--output-format`. |
| **Model override** | Default `gpt-5.5`; override with `--model gpt-5.4`. |
| **Standalone auth** | Own app home at `~/.magi-gen/`. Never depends on `magi-code` or `~/.mc/` except optional explicit `import magi-code`. |
| **Secure by default** | Auth files locked to `0600`, symlinks rejected, all secrets redacted in output and errors. |

## Quick start

Requirements: Rust 1.88.0+ with Cargo, network access to OpenAI/Codex backend.

Install from crates.io:

```sh
cargo install magi-gen
```

Login with Codex/ChatGPT subscription OAuth (opens browser):

```sh
magi-gen login codex
```

Generate an image:

```sh
magi-gen "cyberpunk raccoon eating ramen" -o raccoon.png
```

Shorthand with derived filename:

```sh
magi-gen "red circle on white background"
# writes red-circle-on-white-background.png
```

High quality WebP:

```sh
magi-gen generate "neon city skyline at night" --quality high --output-format webp -o city.webp
```

### OpenAI-compatible API

For providers using API keys instead of subscription OAuth:

```sh
export OPENAI_API_KEY=sk-...
magi-gen generate "red circle on white background" \
  --provider openai-compatible \
  --base-url https://api.openai.com/v1 \
  --api-key-env OPENAI_API_KEY \
  --model gpt-5.5 \
  --output out.png
```

Print base64 to stdout instead of writing file:

```sh
magi-gen generate "small icon" --base64
```

## Transparent backgrounds

The `--transparent` flag produces images with the background removed, suitable for icons, logos, and compositing:

```sh
magi-gen "minimalist app icon, blue lightning bolt" --transparent -o icon.png
```

**How it works:**

1. Appends a system prompt instruction telling the model to generate a **solid bright pink background** (`#FF0096`-like).
2. After generation, samples the **4 corner pixels** to detect the dominant background color.
3. Runs `rustychroma::remove_range()` — a soft chroma key using BT.601 color distance — to strip the background while preserving edge detail.
4. Outputs two files:
   - `icon.png` — transparent **RGBA PNG** with alpha channel
   - `icon-original.png` — original **RGB PNG** with pink background intact

```sh
magi-gen "a cartoon robot mascot" --transparent -o robot.png
# detected transparent background color #F0107B
# wrote robot.png          (RGBA, transparent)
# wrote robot-original.png (RGB, original)
```

> **Note:** Transparent mode forces PNG output format (required for alpha channel). If you pass `--output-format webp` with `--transparent`, a warning is printed and PNG is used.

## Commands

| Command | Description |
| --- | --- |
| `magi-gen "prompt" [options]` | Shorthand generation (no subcommand needed) |
| `magi-gen generate "prompt" [options]` | Explicit generate subcommand |
| `magi-gen login codex` | OAuth login via browser (PKCE flow) |
| `magi-gen logout codex` | Remove stored Codex credentials |
| `magi-gen auth status` | Check if Codex auth is configured |
| `magi-gen import magi-code` | Import credentials from `~/.mc/auth.json` (maps `openai-codex` → `codex`) |

## Options

| Flag | Default | Values | Notes |
| --- | --- | --- | --- |
| `-o, --output <PATH>` | derived from prompt | file path | Slugified from prompt if omitted |
| `--model <MODEL>` | `gpt-5.5` | string | Override to `gpt-5.4` etc. |
| `--size <SIZE>` | `1024x1024` | string | Passed to provider; invalid sizes may be rejected |
| `--quality <QUALITY>` | `low` | `low\|medium\|high\|auto` | `low` is fastest/cheapest |
| `--output-format <FMT>` | `png` | `png\|webp\|jpeg` | Forced to `png` when `--transparent` |
| `--transparent` | off | flag | Chromakey background removal, outputs RGBA PNG + original |
| `--base64` | off | flag | Print base64 to stdout instead of writing file |
| `--provider` | `codex` | `codex\|openai-compatible` | Selects auth method |
| `--base-url <URL>` | — | URL root | Required for `openai-compatible` (e.g. `https://api.openai.com/v1`) |
| `--api-key-env <VAR>` | `OPENAI_API_KEY` | env var name | Which env var holds the API key |

## Files and configuration

Default app home:

```text
~/.magi-gen/
├── auth.json       # OAuth credentials (0600 permissions)
├── settings.json   # Provider configuration
└── cache/
```

Override app home:

```sh
MAGI_GEN_HOME=/custom/path magi-gen auth status
```

The system prompt lives in `prompts/system.md` and is compiled in at build time via `include_str!`. Edit it to iterate on generation behavior without touching Rust code.

## Security

- Auth files locked to **owner-only `0600`** on Unix
- **Symlinked auth files rejected** before read
- **Never reads `~/.mc/`** except explicit `import magi-code` command
- OAuth callback **state validation** prevents CSRF
- All secrets **redacted** in output: bearer tokens, refresh tokens, account IDs, API keys (`sk-*`), OAuth codes
- **Atomic writes** for auth file updates (temp file + rename)

## License

[MIT](LICENSE)
