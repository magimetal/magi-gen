pub mod codex;
pub mod openai_compatible;
pub mod request;
pub mod sse;

use request::{ImageRequest, ImageResult};

pub const CODEX_PROVIDER: &str = "codex";
pub const DEFAULT_CODEX_MODEL: &str = "gpt-5.5";
pub const CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";

pub trait ImageProvider {
    fn generate(&self, request: ImageRequest) -> anyhow::Result<ImageResult>;
}
