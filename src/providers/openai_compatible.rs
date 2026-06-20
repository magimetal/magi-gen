use crate::providers::{
    ImageProvider,
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
use url::Url;

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleImageProvider {
    api_key: String,
    base_url: String,
    client: Client,
}

impl OpenAiCompatibleImageProvider {
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>) -> anyhow::Result<Self> {
        let base_url = base_url.into();
        let _ = validate_base_url(&base_url)?;
        Ok(Self {
            api_key: api_key.into(),
            base_url,
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()?,
        })
    }

    pub fn responses_url(&self) -> anyhow::Result<String> {
        responses_url_from_base(&self.base_url)
    }

    pub fn headers(&self) -> BTreeMap<String, String> {
        openai_compatible_sse_headers(&self.api_key)
    }
}

impl ImageProvider for OpenAiCompatibleImageProvider {
    fn generate(&self, request: ImageRequest) -> anyhow::Result<ImageResult> {
        let url = self.responses_url()?;
        let mut builder = self.client.post(url);
        for (name, value) in self.headers() {
            builder = builder.header(name, value);
        }
        let response = builder
            .json(&request.body())
            .send()
            .map_err(redact_api_keys_from_error)?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().map_err(redact_api_keys_from_error)?;
            anyhow::bail!(redact_api_keys(&format!(
                "OpenAI-compatible image request failed with status {status}: {body}"
            )));
        }

        let mut parser = SseImageParser::default();
        let reader = BufReader::new(response);
        for line in reader.lines() {
            let mut line = line
                .map_err(redact_api_keys_from_error)
                .context("could not read OpenAI-compatible SSE stream")?;
            line.push('\n');
            parser
                .push_chunk(&line)
                .map_err(redact_api_keys_from_error)?;
        }
        parser.finish().map_err(redact_api_keys_from_error)
    }
}

pub fn openai_compatible_sse_headers(api_key: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("accept".to_string(), "text/event-stream".to_string()),
        ("content-type".to_string(), "application/json".to_string()),
        (
            "user-agent".to_string(),
            format!("magi-gen/{}", env!("CARGO_PKG_VERSION")),
        ),
        ("authorization".to_string(), format!("Bearer {api_key}")),
    ])
}

fn redact_api_keys_from_error(error: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!(redact_api_keys(&error.to_string()))
}

fn redact_api_keys(message: &str) -> String {
    message
        .split_whitespace()
        .map(|part| {
            if part.starts_with("sk-") {
                "[REDACTED]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn responses_url_from_base(base_url: &str) -> anyhow::Result<String> {
    let url = validate_base_url(base_url)?;
    Ok(format!("{}/responses", url.as_str().trim_end_matches('/')))
}

pub fn validate_base_url(base_url: &str) -> anyhow::Result<Url> {
    let url = Url::parse(base_url).context("base_url must be an absolute URL")?;
    let host = url.host_str().unwrap_or_default();
    let localhost = matches!(host, "localhost" | "127.0.0.1" | "::1");
    if url.scheme() != "https" && !(url.scheme() == "http" && localhost) {
        anyhow::bail!("base_url must use https, except localhost/loopback http")
    }
    if !url.username().is_empty() || url.password().is_some() {
        anyhow::bail!("base_url must not include credentials")
    }
    if url.query().is_some() || url.fragment().is_some() {
        anyhow::bail!("base_url must not include query or fragment")
    }
    let path = url.path().trim_end_matches('/');
    for suffix in ["/responses", "/models", "/completions", "/chat/completions"] {
        if path.ends_with(suffix) {
            anyhow::bail!("base_url must be API root, not endpoint URL ending {suffix}")
        }
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_builds_correct_request_url() {
        let provider = OpenAiCompatibleImageProvider::new("sk-test", "https://api.openai.com/v1")
            .expect("provider builds");

        assert_eq!(
            provider.responses_url().unwrap(),
            "https://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn provider_headers_include_bearer_auth_and_sse_accept() {
        let provider = OpenAiCompatibleImageProvider::new("sk-test", "https://api.openai.com/v1")
            .expect("provider builds");
        let headers = provider.headers();

        assert_eq!(headers["accept"], "text/event-stream");
        assert_eq!(headers["content-type"], "application/json");
        assert_eq!(headers["authorization"], "Bearer sk-test");
    }

    #[test]
    fn api_key_redaction_masks_sk_tokens() {
        let message = redact_api_keys("request failed for sk-secret-token at provider");

        assert_eq!(message, "request failed for [REDACTED] at provider");
    }

    #[test]
    fn base_url_rejects_endpoint_suffixes() {
        for url in [
            "https://api.openai.com/v1/responses",
            "https://api.openai.com/v1/models",
            "https://api.openai.com/v1/completions",
            "https://api.openai.com/v1/chat/completions",
        ] {
            let error = validate_base_url(url).unwrap_err().to_string();
            assert!(error.contains("API root"), "{error}");
        }
    }

    #[test]
    fn base_url_accepts_https_root_and_localhost_http() {
        assert_eq!(
            responses_url_from_base("https://api.openai.com/v1").unwrap(),
            "https://api.openai.com/v1/responses"
        );
        assert_eq!(
            responses_url_from_base("http://localhost:8080/v1").unwrap(),
            "http://localhost:8080/v1/responses"
        );
    }

    #[test]
    fn base_url_rejects_query_fragment_and_credentials() {
        for url in [
            "https://user:pass@example.test/v1",
            "https://example.test/v1?x=1",
            "https://example.test/v1#frag",
            "http://example.test/v1",
        ] {
            assert!(validate_base_url(url).is_err(), "{url}");
        }
    }
}
