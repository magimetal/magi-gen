# magi-gen

Standalone image generation CLI for Codex/ChatGPT subscription OAuth and OpenAI-compatible Responses APIs.

## Install

From repo checkout:

```bash
cargo install --path .
```

Or build release binary:

```bash
cargo build --release
./target/release/magi-gen --help
```

## Quickstart (Codex / ChatGPT subscription)

Login with standalone Codex OAuth:

```bash
magi-gen login codex
```

Generate image with explicit output:

```bash
magi-gen generate "cyberpunk raccoon eating ramen" --output raccoon.png
```

Generate image with prompt shorthand and derived filename:

```bash
magi-gen "red circle on white background"
# writes red-circle-on-white-background.png
```

Optional import for existing magi-code users:

```bash
magi-gen import magi-code
```

This reads `~/.mc/auth.json` only for explicit `import magi-code`, maps `openai-codex` to local `codex`, then writes local auth.

## Quickstart (OpenAI-compatible API)

Use API root URL, not endpoint URL:

```bash
export OPENAI_API_KEY=sk-...
magi-gen generate "red circle on white background" \
  --provider openai-compatible \
  --base-url https://api.openai.com/v1 \
  --api-key-env OPENAI_API_KEY \
  --model gpt-5.5 \
  --output openai-circle.png
```

Print base64 instead of writing file:

```bash
magi-gen generate "small icon" \
  --provider openai-compatible \
  --base-url https://api.openai.com/v1 \
  --base64
```

## Commands

```bash
magi-gen login codex
magi-gen logout codex
magi-gen auth status
magi-gen import magi-code
magi-gen generate "prompt" [options]
magi-gen "prompt" [-o output.png] [--base64]
```

- `login codex`: standalone OAuth login for Codex/ChatGPT subscription.
- `logout codex`: removes local Codex auth record.
- `auth status`: reports whether local Codex auth is configured.
- `import magi-code`: optional one-shot copy from magi-code auth store.
- `generate`: generates image via selected provider.

## Options

- `--model <MODEL>`: Responses model. Default: `gpt-5.5`.
- `--size <SIZE>`: image size string. Default: `1024x1024`.
- `--quality <QUALITY>`: image quality string. Default: `low`.
- `--output, -o <PATH>`: output image path. If omitted and `--base64` is not set, filename derives from prompt: lowercase slug, max 48 chars, `.png`.
- `--base64`: print base64 image result to stdout instead of writing file.
- `--provider <PROVIDER>`: `codex` or `openai-compatible`. Default: `codex`.
- `--base-url <URL>`: API root for OpenAI-compatible provider, for example `https://api.openai.com/v1`.
- `--api-key-env <VAR>`: env var containing API key. Default: `OPENAI_API_KEY`.

## Files

Default app home:

```text
~/.magi-gen/
├── auth.json
├── settings.json
└── cache/
```

Override app home:

```bash
MAGI_GEN_HOME=/custom/path magi-gen auth status
```

Auth file permissions are owner-only (`0600`) on Unix. Symlinked auth files are rejected. Secrets are not printed in status or error output.
