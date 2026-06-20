<!--THIS IS A GENERATED FILE - DO NOT MODIFY DIRECTLY, FOR MANUAL ADJUSTMENTS UPDATE `AGENTS_CUSTOM.MD`-->
# SRC KNOWLEDGE BASE

## OVERVIEW
`src/` owns CLI dispatch, auth persistence/OAuth, provider HTTP/SSE handling, output decoding, and chromakey transparency.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Subcommand definitions | `cli.rs` | clap derive structs/enums |
| Runtime dispatch | `main.rs` | shorthand prompt path plus subcommands |
| App paths/settings | `config.rs` | `MAGI_GEN_HOME`, default settings |
| Credential import | `import.rs` | only allowed `~/.mc/auth.json` reader |
| Image file writing | `output.rs` | base64 decode, parent dir creation |
| Transparent PNG | `chromakey.rs` | corner color detect + rustychroma removal |
| OAuth/store | `auth/` | login, refresh, auth JSON safety |
| API providers | `providers/` | request body, headers, SSE parser |

## CONVENTIONS
- Keep unit tests beside module code.
- Keep provider-neutral request shape in `providers/request.rs`.
- Keep provider-specific auth/header/url behavior in provider modules.
- Keep auth JSON mutation through `store::update_auth` or `write_auth`.
- Use `anyhow::Context` on file/network boundaries.

## ANTI-PATTERNS
- No `Debug` output for raw secrets; custom redacted `Debug` where records hold tokens.
- No provider body forks unless API shapes diverge; shared `ImageRequest::body` is current contract.
- No direct settings/auth file writes bypassing atomic helpers.
- No endpoint URLs as base URL input.
- No new runtime dependency on magi-code.

## GOTCHAS
- `auth/codex.rs` is large because it contains PKCE, loopback HTTP, token normalization, JWT extraction, redaction, and tests.
- `providers/sse.rs` accepts partial image only when final/completed result never arrives.
- `chromakey.rs` returns detected RGB color for user-facing diagnostic.
- `main.rs` writes original image only when transparent mode is enabled.
