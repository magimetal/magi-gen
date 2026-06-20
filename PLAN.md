# PLAN: magi-image-gen-cli

## Objective
Build standalone Rust CLI `magi-image-gen-cli` that generates images from a prompt using either:

1. ChatGPT/Codex subscription OAuth against Codex backend, using `model: "gpt-5.5"` plus hosted `image_generation` tool.
2. OpenAI-compatible Responses API using `base_url` + API key.

Standalone requirement: auth must work on machines without `magi-code`. Reuse protocol/implementation ideas from magi-code source, but do not depend on `~/.mc` or magi-code crate at runtime.

## Known Working Discovery
Live test proved Codex image generation works:

- Endpoint: `https://chatgpt.com/backend-api/codex/responses`
- Model field: `gpt-5.5`
- Tool: `{"type":"image_generation"}`
- Returned SSE events including:
  - `response.image_generation_call.partial_image`
  - `response.output_item.done` with `item.type == "image_generation_call"`
  - `response.completed`
- Returned base64 decoded to valid PNG.
- Test output saved at `/tmp/magi-code-codex-image-test.png` on source machine.

Important negative result:

- Direct `model: "gpt-image-2"` on Codex backend failed with:
  - `The 'gpt-image-2' model is not supported when using Codex with a ChatGPT account.`
- Therefore use `gpt-5.5` as Responses `model`; GPT Image 2 is backend image model selected by hosted image tool.

Codex backend differs from public `/v1/responses`:

- `input` must be list of message objects.
- Public-style string input returned `Input must be a list`.

## Target UX

```bash
# First-time standalone auth
magi-image-gen-cli login codex

# Generate with default Codex provider
magi-image-gen-cli "red circle on white background" -o circle.png

# Explicit generate command
magi-image-gen-cli generate "cyberpunk raccoon eating ramen" --output raccoon.png

# Custom OpenAI-compatible Responses provider
OPENAI_API_KEY=sk-...
magi-image-gen-cli generate "prompt" \
  --provider openai-compatible \
  --base-url https://api.openai.com/v1 \
  --api-key-env OPENAI_API_KEY \
  --model gpt-5.5 \
  --output out.png

# Auth management
magi-image-gen-cli auth status
magi-image-gen-cli logout codex

# Optional existing-user convenience, not required for standalone
magi-image-gen-cli import magi-code
```

## App Home / Files

Use own app home, not magi-code `~/.mc`:

```text
~/.magi-image-gen-cli/
├── auth.json
├── settings.json
└── cache/
```

Support override:

```bash
MAGI_IMAGE_GEN_HOME=/custom/path magi-image-gen-cli ...
```

### auth.json shape

```json
{
  "providers": {
    "codex": {
      "type": "oauth",
      "access": "...",
      "refresh": "...",
      "expires": 1782672820,
      "account_id": "..."
    }
  }
}
```

Requirements:

- File permissions: `0600` or stricter on Unix.
- Reject symlinked auth files before reading.
- Never print `access`, `refresh`, `account_id`, bearer header, or full OAuth redirect contents.
- Atomic writes for auth updates.

### settings.json initial shape

```json
{
  "default_provider": "codex",
  "codex": {
    "model": "gpt-5.5"
  },
  "openai_compatible": {
    "base_url": "https://api.openai.com/v1",
    "api_key_env_var": "OPENAI_API_KEY",
    "model": "gpt-5.5"
  }
}
```

Settings are required for MVP; CLI flags override settings. Keep schema small.

## Rust Project Layout

```text
magi-image-gen-cli/
├── Cargo.toml
├── README.md
├── PLAN.md
└── src/
    ├── main.rs
    ├── cli.rs
    ├── config.rs
    ├── output.rs
    ├── auth/
    │   ├── mod.rs
    │   ├── store.rs
    │   └── codex.rs
    └── providers/
        ├── mod.rs
        ├── codex.rs
        ├── openai_compatible.rs
        ├── request.rs
        ├── sse.rs
        └── transport.rs
```

## Cargo Dependencies

Start with:

```toml
[package]
name = "magi-image-gen-cli"
version = "0.1.0"
edition = "2024"
rust-version = "1.88.0"
publish = false

[dependencies]
anyhow = "1.0"
base64 = "0.22"
chrono = { version = "0.4", default-features = false, features = ["clock", "serde", "std"] }
clap = { version = "4.5", features = ["derive"] }
dirs = "5.0"
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = "0.10"
thiserror = "2.0"
urlencoding = "2.1"
uuid = { version = "1", features = ["v4"] }

[dev-dependencies]
tempfile = "3"
```

Add `open` crate only if implementing automatic browser open. Printing URL is enough for MVP.

## magi-code Source References

Assume next agent has access to magi-code repo at current worktree or can locate it.

### OAuth / Codex login
Reference file: `magi-code/src/login.rs`

Key items to copy/adapt:

- Constants:
  - `OPENAI_CODEX_CLIENT_ID`
  - `AUTHORIZE_URL`
  - `TOKEN_URL`
  - `REDIRECT_URI`
  - `CALLBACK_ADDR`
  - `SCOPE`
  - `REFRESH_SKEW_SECS`
- PKCE flow:
  - `OAuthAttempt::new`
  - `OAuthAttempt::authorization_url`
  - `random_urlsafe`
  - SHA-256 + base64 URL-safe no-pad challenge
- Login flow:
  - `login_openai_codex`
  - `login_openai_codex_with_controls`
  - `capture_loopback_or_manual_code`
  - `handle_callback_stream`
  - `parse_manual_fallback_input`
  - `parse_redirect_target`
- Token exchange/refresh:
  - `exchange_code`
  - `refresh_token`
  - `post_token_form`
  - `normalize_token_response`
- Stored credential handling:
  - `codex_credential_from_store`
  - `force_refresh_codex_credential_from_store`
  - `persist_token`

Implementation note: rename provider id from magi-code `openai-codex` to app-local `codex`, unless compatibility import requires preserving old id.

### Auth store / redaction / permissions
Reference files:

- `magi-code/src/config/auth.rs`
- `magi-code/src/config/paths.rs`

Copy/adapt:

- `Auth`, `AuthProviderRecord`, `ProviderCredential` concepts.
- `read_auth`, `write_auth`, `update_auth`.
- `validate_auth_file_before_read`: reject symlink, enforce owner-only permissions.
- `extract_chatgpt_account_id_from_jwt` / account id fallback.
- `McPaths::resolve` pattern, but use `MAGI_IMAGE_GEN_HOME` and `~/.magi-image-gen-cli`.

Do not copy broad magi-code config types unless needed.

### Codex provider request
Reference files:

- `magi-code/src/providers/mod.rs`
- `magi-code/src/providers/openai/codex.rs`
- `magi-code/src/providers/openai/headers.rs`
- `magi-code/src/providers/openai/bodies.rs`

Relevant constants/patterns:

- `DEFAULT_CODEX_MODEL: "gpt-5.5"`
- `CODEX_RESPONSES_URL: "https://chatgpt.com/backend-api/codex/responses"`
- `CODEX_MODELS_URL: "https://chatgpt.com/backend-api/codex/models"` optional.
- `codex_sse_headers`:
  - `accept: text/event-stream`
  - `content-type: application/json`
  - `user-agent`
  - `authorization: Bearer <access>`
  - `chatgpt-account-id: <account_id>`
  - `originator: pi`
  - `openai-beta: responses=experimental`
- `codex_responses_body` for required Codex shape.

For this app, do not bring agent/tool schemas. Build image-only body directly.

### OpenAI-compatible provider
Reference files:

- `magi-code/src/providers/openai/compatible.rs`
- `magi-code/src/config/custom_provider_config.rs`

Copy/adapt:

- Append endpoint paths to API root:
  - `{base_url}/responses`
  - optional `{base_url}/models`
- Headers:
  - `accept: text/event-stream`
  - `content-type: application/json`
  - optional `authorization: Bearer <api_key>`
- URL validation:
  - HTTPS required except localhost/loopback HTTP.
  - Reject credentials/userinfo, query, fragment.
  - Reject endpoint URLs ending `/responses`, `/models`, `/completions`, `/chat/completions`; require API root.

Note: OpenAI-compatible providers may not implement hosted `image_generation`; emit clear error if SSE/API rejects tool.

### Streaming / transport
Reference files:

- `magi-code/src/providers/transport.rs`
- `magi-code/src/providers/openai_stream.rs`
- `magi-code/src/providers/stream.rs`

For this app, implement smaller parser:

- Read HTTP response line-by-line.
- Only process lines beginning `data: `.
- Stop on `[DONE]`.
- Parse JSON.
- Capture image result from these event shapes:
  - `type == "response.image_generation_call.partial_image"` → `partial_image_b64`
  - `type == "response.output_item.done"` and `item.type == "image_generation_call"` → `item.result`
  - `type == "response.completed"`, scan `response.output[]` for `type == "image_generation_call"` → `result`
- Prefer final result over partial image.
- If text output exists but no image, print text as diagnostic and return non-zero.

## Request Body Builder

Use this for both Codex and compatible Responses API initially:

```rust
serde_json::json!({
    "model": model,
    "store": false,
    "stream": true,
    "instructions": "Use the image_generation tool. No text unless image generation is unavailable.",
    "input": [{
        "role": "user",
        "content": prompt,
    }],
    "tools": [{
        "type": "image_generation",
        "action": "generate",
        "quality": quality,
        "size": size,
        "partial_images": 1,
    }],
    "tool_choice": { "type": "image_generation" },
    "text": { "verbosity": "low" }
})
```

Flags:

- `--model`, default `gpt-5.5`.
- `--size`, default `1024x1024`.
- `--quality`, default `low` for cheap fast default. Allow `low|medium|high|auto` as strings; do not over-validate until provider rejects.
- `--output/-o`, default derived slug + `.png` or required for MVP.
- `--base64`, print base64 instead of writing file.
- `--provider codex|openai-compatible`.

Do not expose `gpt-image-2` as Codex model. If adding image model selection later, pass it only through provider-supported tool options if docs/API expose one.

## CLI Types Sketch

```rust
#[derive(clap::Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Prompt shorthand when no subcommand is provided.
    prompt: Option<String>,

    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(clap::Subcommand)]
enum Command {
    Generate(GenerateArgs),
    Login { provider: LoginProvider },
    Logout { provider: LoginProvider },
    Auth { #[command(subcommand)] command: AuthCommand },
    Import { source: ImportSource },
}

#[derive(clap::Args)]
struct GenerateArgs {
    prompt: String,
    #[arg(long, default_value = "codex")]
    provider: ProviderKind,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long, default_value = "gpt-5.5")]
    model: String,
    #[arg(long, default_value = "1024x1024")]
    size: String,
    #[arg(long, default_value = "low")]
    quality: String,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    api_key_env: Option<String>,
    #[arg(long)]
    base64: bool,
}
```

## Provider Trait Sketch

```rust
trait ImageProvider {
    fn generate(&self, request: ImageRequest) -> anyhow::Result<ImageResult>;
}

struct ImageRequest {
    prompt: String,
    model: String,
    size: String,
    quality: String,
}

struct ImageResult {
    base64: String,
    mime_hint: Option<String>,
}
```

Implement:

- `CodexImageProvider`
- `OpenAiCompatibleImageProvider`

## Error Handling Requirements

- Redact any secret-looking values from errors:
  - bearer tokens
  - access/refresh token fields
  - account ids
  - API keys starting `sk-`
  - OAuth codes and callback query strings
- On missing Codex auth:
  - `Not logged in. Run: magi-image-gen-cli login codex`
- On expired token without refresh:
  - `Codex OAuth expired. Run: magi-image-gen-cli login codex`
- On compatible provider missing env var:
  - `Missing API key env var OPENAI_API_KEY`
- On no image result:
  - show event types and any text output, not raw full response with secrets.

## Implementation Phases

### Phase 1 — Skeleton + Codex generation

Files:

- `Cargo.toml`
- `src/main.rs`
- `src/cli.rs`
- `src/config.rs`
- `src/auth/store.rs`
- `src/providers/codex.rs`
- `src/providers/sse.rs`
- `src/output.rs`

Scope:

- CLI parses prompt and `-o`.
- Reads standalone auth from `~/.magi-image-gen-cli/auth.json`.
- Sends Codex image request.
- Parses SSE final/partial image.
- Writes PNG bytes.

Temporary acceptance: manually place valid auth JSON copied from magi-code to test generation. This validates provider before implementing login.

### Phase 2 — Standalone login/refresh/logout

Files:

- `src/auth/codex.rs`
- `src/auth/store.rs`

Scope:

- Implement PKCE login.
- Local callback + manual paste fallback.
- Token exchange + refresh.
- Persist `0600` auth.
- Auto-refresh before generation.
- `logout codex` removes record.

Acceptance:

```bash
trash ~/.magi-image-gen-cli
magi-image-gen-cli login codex
magi-image-gen-cli "red circle on white background" -o /tmp/circle.png
file /tmp/circle.png
```

### Phase 3 — OpenAI-compatible provider

Files:

- `src/providers/openai_compatible.rs`
- config/CLI additions

Scope:

- `--provider openai-compatible`
- `--base-url`
- `--api-key-env`
- append `/responses`
- same image request body
- same SSE parser

Acceptance:

```bash
OPENAI_API_KEY=...
magi-image-gen-cli generate "red circle" \
  --provider openai-compatible \
  --base-url https://api.openai.com/v1 \
  --api-key-env OPENAI_API_KEY \
  -o /tmp/openai-circle.png
```

If API rejects hosted image tool, return clean provider error.

### Phase 4 — Polish

- README quickstart.
- `auth status`.
- `import magi-code` optional.
- Filename derivation if `-o` omitted.
- `--base64` output.
- Tests for URL validation, auth permissions, SSE parsing fixtures, redaction.

## Verification Commands

```bash
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings
cargo run -- --help
cargo run -- login codex
cargo run -- "red circle on white background" -o /tmp/magi-image-test.png
file /tmp/magi-image-test.png
```

Expected `file` output should identify PNG image data.

## Test Fixtures

Add unit tests with synthetic SSE:

```text
data: {"type":"response.image_generation_call.partial_image","partial_image_b64":"..."}

data: {"type":"response.output_item.done","item":{"type":"image_generation_call","status":"generating","result":"..."}}

data: {"type":"response.completed","response":{"output":[{"type":"image_generation_call","result":"..."}]}}

data: [DONE]
```

Assertions:

- Final result beats partial result.
- Partial result accepted if final missing.
- No image + text response returns error with text diagnostic.
- Malformed data line ignored or reported depending parser mode.

## Do Not Do

- Do not require magi-code installed.
- Do not read `~/.mc/auth.json` except optional explicit `import magi-code` command.
- Do not use `gpt-image-2` as Codex Responses `model`.
- Do not print tokens, account ids, auth codes, bearer headers, or full OAuth callback URLs.
- Do not add TUI, sessions, tools, model catalog, or agent runtime. This CLI only generates images.

## Open Questions

- Whether OpenAI-compatible providers support hosted `image_generation` tool consistently. Treat as provider capability, not guaranteed.
- Whether output format can be requested beyond PNG on Codex. Prior rejected `tools[0].format`; do not include `format` initially.
- Whether quality/size enum differs across providers. Pass strings through and surface provider error.

## Executor Notes

- Start by copying minimal patterns from magi-code, then delete unrelated agent abstractions.
- Keep modules small. No generic provider framework beyond two providers.
- Use blocking reqwest, not async, for simpler CLI.
- Use `python3` only for ad hoc external validation if needed; Rust tests should cover parser/auth helpers.
