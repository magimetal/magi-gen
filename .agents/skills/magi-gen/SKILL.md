# magi-gen

Generate images from text prompts using OpenAI Codex/ChatGPT subscription OAuth or any OpenAI-compatible Responses API.

## When to Use

Use when the user wants to generate images from text prompts. Assumes `magi-gen` is installed and logged in (`magi-gen login codex`).

## Commands

### Basic generation (shorthand)

```bash
magi-gen "your prompt here" -o output.png
```

### Explicit generate subcommand

```bash
magi-gen generate "your prompt here" -o output.png
```

### Transparent background

```bash
magi-gen "a cartoon logo icon" --transparent -o logo.png
```

Produces two files:
- `logo.png` — transparent RGBA PNG (background removed via chromakey)
- `logo-original.png` — original RGB PNG with solid pink background

The `--transparent` flag injects a prompt instruction telling the model to use a solid bright pink background, then removes it post-generation using corner color detection + chromakey.

## Options

| Flag | Default | Values | Notes |
|------|---------|--------|-------|
| `-o, --output <PATH>` | derived from prompt | path | Filename slugified from prompt if omitted |
| `--model <MODEL>` | `gpt-5.5` | string | Use `gpt-5.4` to override |
| `--size <SIZE>` | `1024x1024` | string | Pass through, provider may reject invalid sizes |
| `--quality <QUALITY>` | `low` | `low\|medium\|high\|auto` | `low` is fastest/cheapest |
| `--output-format <FMT>` | `png` | `png\|webp\|jpeg` | webp ~30% smaller |
| `--transparent` | off | flag | Forces png, saves both chromakeyed + original |
| `--base64` | off | flag | Print base64 to stdout instead of writing file |
| `--provider` | `codex` | `codex\|openai-compatible` | Codex uses subscription OAuth |
| `--base-url <URL>` | none | URL root | Required for openai-compatible (e.g. `https://api.openai.com/v1`) |
| `--api-key-env <VAR>` | `OPENAI_API_KEY` | env var name | For openai-compatible provider |

## Auth Management

```bash
magi-gen login codex        # OAuth login via browser
magi-gen logout codex       # Remove stored credentials
magi-gen auth status        # Check if configured
magi-gen import magi-code   # Import creds from ~/.mc/auth.json
```

Auth stored at `~/.magi-gen/auth.json` with `0600` permissions. Override home with `MAGI_GEN_HOME`.

## Examples

### Icon/logo with transparency
```bash
magi-gen "minimalist app icon, a blue lightning bolt" --transparent -o app-icon.png
```

### High quality webp
```bash
magi-gen generate "cyberpunk city at night, neon signs" --quality high --output-format webp -o city.webp
```

### Quick sketch
```bash
magi-gen "rough sketch of a dog" -o dog.png
```

### Different model
```bash
magi-gen "abstract art" --model gpt-5.4 -o art.png
```

## System Prompt

The system prompt lives in `prompts/system.md` and is compiled in via `include_str!`. Edit that file to iterate on prompt behavior without touching Rust code.

## Files

```
~/.magi-gen/
├── auth.json       # OAuth credentials (0600)
├── settings.json   # Provider config
└── cache/
```
