# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-06-19

### Added
- Standalone image generation CLI with two providers:
  - **Codex** (default): ChatGPT subscription OAuth via PKCE login, token auto-refresh
  - **OpenAI-compatible**: API key + `--base-url` against any Responses API endpoint
- SSE streaming parser for `image_generation` tool (partial + final image capture)
- Standalone auth store at `~/.magi-image-gen-cli/` with `0600` permissions, symlink rejection, atomic writes
- Secret redaction (tokens, account IDs, API keys) in all output and error paths
- `login codex` / `logout codex` / `auth status` commands
- `import magi-code` optional credential migration from `~/.mc/auth.json`
- Filename derivation from prompt when `-o` omitted (slugified + extension)
- `--base64` stdout output mode
- `--output-format png|webp|jpeg` flag (default png)
- `--model` flag for model override (default `gpt-5.5`)
- `--size` flag (default `1024x1024`)
- `--quality` flag (default `low`)
- `--transparent` flag with chromakey background removal:
  - Injects solid bright pink background instruction into system prompt
  - Detects background color from 4 corner pixels
  - Uses `rustychroma::remove_range()` for soft chroma key removal
  - Outputs transparent RGBA PNG + original RGB copy (`*-original.png`)
- System prompt extracted to `prompts/system.md` (compiled via `include_str!`)
- `MAGI_IMAGE_GEN_HOME` environment variable for custom app home
- `settings.json` for provider configuration
- 44 unit tests covering SSE parsing, auth, config, URL validation, chromakey, CLI

### Security
- Auth files locked to `0600` on Unix
- Symlinked auth files rejected before read
- Never reads `~/.mc/` except explicit `import magi-code` command
- OAuth callback state validation
- Bearer tokens, refresh tokens, account IDs, API keys redacted in all paths
