<!--THIS IS A GENERATED FILE - DO NOT MODIFY DIRECTLY, FOR MANUAL ADJUSTMENTS UPDATE `AGENTS_CUSTOM.MD`-->
# ALWAYS READ THESE FILE(S)
- @AGENTS_CUSTOM.md

# PROJECT KNOWLEDGE BASE

**Generated:** 2026-06-20T03:05:31Z
**Commit:** c703ff7
**Branch:** main

## OVERVIEW
magi-gen is Rust 2024 single-crate CLI for image generation through Codex/ChatGPT OAuth or OpenAI-compatible Responses APIs. Core risks: auth secrecy, SSE image parsing, chromakey transparent output.

## STRUCTURE
```text
./
├── src/                    # CLI, auth, providers, output/chromakey
├── prompts/system.md       # compile-time image tool instruction
├── .agents/skills/magi-gen # user-facing local skill docs
├── Cargo.toml              # package name magi-gen, Rust 1.88+
└── README.md               # public CLI contract
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| CLI args/subcommands | `src/cli.rs`, `src/main.rs` | Shorthand prompt path lives in `Command::None` branch |
| Codex OAuth/login | `src/auth/codex.rs`, `src/auth/store.rs` | PKCE, loopback callback, refresh, 0600 auth writes |
| Provider requests | `src/providers/` | Shared request body + provider-specific headers/URLs |
| Streaming image parsing | `src/providers/sse.rs` | partial/final/completed image result precedence |
| Transparent output | `src/chromakey.rs`, `src/main.rs` | forces PNG, writes `*-original` copy |
| Output files/base64 | `src/output.rs` | base64 decode + file write |
| Prompt behavior | `prompts/system.md` | included via `include_str!`; rebuild needed |
| Local user docs | `.agents/skills/magi-gen/SKILL.md` | mirrors installed CLI usage |

## CODE MAP
| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `run` | fn | `src/main.rs` | top-level subcommand dispatch |
| `generate_codex` | fn | `src/main.rs` | Codex provider generation path |
| `generate_openai_compatible` | fn | `src/main.rs` | API-key provider generation path |
| `write_generate_result` | fn | `src/main.rs` | base64/stdout/file + transparent original output |
| `AppPaths` | struct | `src/config.rs` | `~/.magi-gen` / `MAGI_GEN_HOME` paths |
| `ImageProvider` | trait | `src/providers/mod.rs` | provider abstraction |
| `ImageRequest::body` | fn | `src/providers/request.rs` | Responses API JSON body |
| `SseImageParser` | struct | `src/providers/sse.rs` | streaming event parser |
| `transparent_png_base64` | fn | `src/chromakey.rs` | chromakey RGBA PNG conversion |

## CONVENTIONS
- Binary/package name is `magi-gen`; old `magi-image-gen-cli` survives only in historical docs/repo context.
- App home is `~/.magi-gen`; override is `MAGI_GEN_HOME`.
- Default Codex Responses model is `gpt-5.5`; reject `gpt-image-2` for Codex.
- Public/OpenAI-compatible provider needs API root `--base-url`, not endpoint URL.
- Tests live inline in each Rust module; `tests/` currently empty.
- `prompts/system.md` changes alter request body through compile-time include.

## ANTI-PATTERNS (THIS PROJECT)
- Do not read `~/.mc/` except explicit `import magi-code` command.
- Do not log access tokens, refresh tokens, OAuth codes, account IDs, or `sk-*` keys.
- Do not accept symlinked auth files.
- Do not write auth/settings without owner-only `0600` on Unix.
- Do not send string `input` to Codex backend; it requires message list shape.
- Do not let `--transparent` emit WebP/JPEG; force PNG.

## UNIQUE STYLES
- Errors prefer actionable CLI remediation: `Run: magi-gen login codex`.
- Provider streams read line-by-line, append newline, feed shared SSE parser.
- Security structs implement custom `Debug` with redacted fields.
- URL validators reject credentials, query, fragment, endpoint suffixes.

## COMMANDS
```bash
cargo test
cargo build
cargo run -- "red circle on white background" -o circle.png
cargo run -- login codex
cargo run -- auth status
MAGI_GEN_HOME=/tmp/magi-gen-test cargo run -- auth status
```

## NOTES
- `AGENTS_CUSTOM.md` is human-maintained; preserve it.
- `rustychroma` API quirks are documented in `AGENTS_CUSTOM.md`.
- Example PNGs at repo root are artifacts, not source contract.
