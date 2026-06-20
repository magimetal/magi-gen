use crate::providers::{
    CODEX_RESPONSES_URL, ImageProvider,
    request::{ImageRequest, ImageResult},
    sse::SseImageParser,
};
use anyhow::Context;
use reqwest::blocking::Client;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader},
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct CodexImageProvider {
    access_token: String,
    account_id: String,
    client: Client,
}

impl CodexImageProvider {
    pub fn new(
        access_token: impl Into<String>,
        account_id: impl Into<String>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            access_token: access_token.into(),
            account_id: account_id.into(),
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()?,
        })
    }

    pub fn headers(&self) -> BTreeMap<String, String> {
        codex_sse_headers(&self.access_token, &self.account_id)
    }
}

impl ImageProvider for CodexImageProvider {
    fn generate(&self, request: ImageRequest) -> anyhow::Result<ImageResult> {
        let mut builder = self.client.post(CODEX_RESPONSES_URL);
        for (name, value) in self.headers() {
            builder = builder.header(name, value);
        }
        let response = builder.json(&request.body()).send()?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("Codex image request failed with status {status}");
        }

        let mut parser = SseImageParser::default();
        let reader = BufReader::new(response);
        for line in reader.lines() {
            let mut line = line.context("could not read Codex SSE stream")?;
            line.push('\n');
            parser.push_chunk(&line)?;
        }
        parser.finish()
    }
}

pub fn codex_sse_headers(token: &str, account_id: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("accept".to_string(), "text/event-stream".to_string()),
        ("content-type".to_string(), "application/json".to_string()),
        (
            "user-agent".to_string(),
            format!("magi-gen/{}", env!("CARGO_PKG_VERSION")),
        ),
        ("authorization".to_string(), format!("Bearer {token}")),
        ("chatgpt-account-id".to_string(), account_id.to_string()),
        ("originator".to_string(), "pi".to_string()),
        (
            "openai-beta".to_string(),
            "responses=experimental".to_string(),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_headers_match_required_contract() {
        let headers = codex_sse_headers("token", "acct");

        assert_eq!(headers["accept"], "text/event-stream");
        assert_eq!(headers["content-type"], "application/json");
        assert_eq!(headers["authorization"], "Bearer token");
        assert_eq!(headers["chatgpt-account-id"], "acct");
        assert_eq!(headers["originator"], "pi");
        assert_eq!(headers["openai-beta"], "responses=experimental");
    }
}
