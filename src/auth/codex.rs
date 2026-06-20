use crate::{auth::store, config::AppPaths, providers::CODEX_PROVIDER};
use anyhow::Context;
use base64::Engine;
use chrono::Utc;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fmt,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    time::{Duration, Instant},
};

pub(crate) const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub(crate) const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
pub(crate) const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub(crate) const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
pub(crate) const CALLBACK_ADDR: &str = "127.0.0.1:1455";
pub(crate) const SCOPE: &str = "openid profile email offline_access";
pub(crate) const REFRESH_SKEW_SECS: i64 = 300;
const LOGIN_WAIT_TIMEOUT: Duration = Duration::from_secs(300);
#[cfg(not(test))]
const CALLBACK_STREAM_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(test)]
const CALLBACK_STREAM_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Clone)]
pub(crate) struct OAuthAttempt {
    verifier: String,
    state: String,
    challenge: String,
}

impl fmt::Debug for OAuthAttempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthAttempt")
            .field("verifier", &"<redacted>")
            .field("state", &"<redacted>")
            .field("challenge", &"<redacted>")
            .finish()
    }
}

impl OAuthAttempt {
    pub(crate) fn new() -> Self {
        let verifier = random_urlsafe(64);
        Self::from_verifier_and_state(verifier, random_urlsafe(32))
    }

    #[cfg(test)]
    fn from_parts(verifier: impl Into<String>, state: impl Into<String>) -> Self {
        Self::from_verifier_and_state(verifier.into(), state.into())
    }

    fn from_verifier_and_state(verifier: String, state: String) -> Self {
        let challenge = pkce_challenge(&verifier);
        Self {
            verifier,
            state,
            challenge,
        }
    }

    pub(crate) fn authorization_url(&self) -> String {
        let pairs = [
            ("response_type", "code"),
            ("client_id", CLIENT_ID),
            ("redirect_uri", REDIRECT_URI),
            ("scope", SCOPE),
            ("code_challenge", self.challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", self.state.as_str()),
            ("id_token_add_organizations", "true"),
            ("codex_cli_simplified_flow", "true"),
            ("originator", "pi"),
        ];
        let query = pairs
            .into_iter()
            .map(|(k, v)| format!("{}={}", pct(k), pct(v)))
            .collect::<Vec<_>>()
            .join("&");
        format!("{AUTHORIZE_URL}?{query}")
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LoginResult {
    pub(crate) message: String,
}

#[derive(Debug, Clone)]
pub(crate) struct NormalizedToken {
    pub(crate) access: String,
    pub(crate) refresh: Option<String>,
    pub(crate) expires: Option<i64>,
    pub(crate) account_id: String,
}

#[derive(Deserialize)]
pub(crate) struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    #[serde(rename = "accountId")]
    account_id_camel: Option<String>,
    account_id: Option<String>,
    id_token: Option<String>,
}

pub(crate) fn login_codex(paths: &AppPaths) -> anyhow::Result<LoginResult> {
    let attempt = OAuthAttempt::new();
    let url = attempt.authorization_url();
    eprintln!("Open this URL in your browser:\n{url}");
    eprintln!(
        "Waiting up to 5 minutes for browser callback on {REDIRECT_URI}. If it times out, paste final redirect URL or authorization code when prompted."
    );
    let code = match capture_loopback_code(&attempt.state, LOGIN_WAIT_TIMEOUT) {
        Ok(code) => code,
        Err(error) => {
            eprintln!(
                "OAuth callback unavailable: {}",
                redact_oauth_text(&error.to_string())
            );
            prompt_manual_code(&attempt.state)?
        }
    };
    let token = exchange_code(&attempt, &code)?;
    persist_token(paths, token)?;
    Ok(LoginResult {
        message: "codex: logged in".to_string(),
    })
}

pub(crate) fn exchange_code(attempt: &OAuthAttempt, code: &str) -> anyhow::Result<NormalizedToken> {
    let body = [
        ("grant_type", "authorization_code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", REDIRECT_URI),
        ("code", code),
        ("code_verifier", attempt.verifier.as_str()),
    ];
    post_token_form(&body)
}

pub(crate) fn refresh_token(refresh: &str) -> anyhow::Result<NormalizedToken> {
    let body = [
        ("grant_type", "refresh_token"),
        ("client_id", CLIENT_ID),
        ("refresh_token", refresh),
    ];
    post_token_form(&body)
}

fn post_token_form(body: &[(&str, &str)]) -> anyhow::Result<NormalizedToken> {
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?
        .post(TOKEN_URL)
        .form(body)
        .send()
        .context("Codex OAuth token request failed")?;
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("Codex OAuth token exchange failed with status {status}")
    }
    normalize_token_response(response.json::<TokenResponse>()?)
}

pub(crate) fn normalize_token_response(response: TokenResponse) -> anyhow::Result<NormalizedToken> {
    let access = response
        .access_token
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("OAuth token response missing access token"))?;
    let account_id = response
        .account_id_camel
        .or(response.account_id)
        .or_else(|| extract_chatgpt_account_id_from_jwt(&access).ok())
        .or_else(|| {
            response
                .id_token
                .as_deref()
                .and_then(|token| extract_chatgpt_account_id_from_jwt(token).ok())
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("OAuth token response missing ChatGPT account id"))?;
    Ok(NormalizedToken {
        access,
        refresh: response.refresh_token.filter(|value| !value.is_empty()),
        expires: response
            .expires_in
            .map(|seconds| Utc::now().timestamp() + seconds),
        account_id,
    })
}

pub(crate) fn persist_token(paths: &AppPaths, token: NormalizedToken) -> anyhow::Result<()> {
    store::update_auth(paths, |auth| {
        let previous_refresh = match auth.providers.get(CODEX_PROVIDER) {
            Some(store::AuthProviderRecord::OAuth { refresh, .. }) => refresh.clone(),
            None => None,
        };
        auth.providers.insert(
            CODEX_PROVIDER.to_string(),
            store::AuthProviderRecord::OAuth {
                access: token.access,
                refresh: token.refresh.or(previous_refresh),
                expires: token.expires,
                account_id: Some(token.account_id),
            },
        );
    })?;
    Ok(())
}

fn capture_loopback_code(expected_state: &str, timeout: Duration) -> anyhow::Result<String> {
    let listener = TcpListener::bind(CALLBACK_ADDR)
        .with_context(|| "could not bind OAuth callback on 127.0.0.1:1455")?;
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => return handle_callback_stream(&mut stream, expected_state),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
        if Instant::now() >= deadline {
            anyhow::bail!("OAuth callback timed out")
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn prompt_manual_code(expected_state: &str) -> anyhow::Result<String> {
    eprint!("Paste redirect URL or authorization code: ");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    parse_manual_fallback_input(&input, expected_state)
}

fn handle_callback_stream(stream: &mut TcpStream, expected_state: &str) -> anyhow::Result<String> {
    stream.set_read_timeout(Some(CALLBACK_STREAM_TIMEOUT))?;
    stream.set_write_timeout(Some(CALLBACK_STREAM_TIMEOUT))?;
    let mut buf = [0_u8; 4096];
    let n = stream.read(&mut buf).map_err(|error| {
        if matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ) {
            anyhow::anyhow!("OAuth callback read timed out")
        } else {
            error.into()
        }
    })?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let first = request.lines().next().unwrap_or_default();
    let result = parse_callback_request_line(first, expected_state);
    let (status, body) = if result.is_ok() {
        ("200 OK", "Codex login complete. You can close this tab.")
    } else {
        (
            "400 Bad Request",
            "Codex login failed. Return to your terminal.",
        )
    };
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    result
}

fn parse_callback_request_line(line: &str, expected_state: &str) -> anyhow::Result<String> {
    let Some(target) = line
        .strip_prefix("GET ")
        .and_then(|rest| rest.split_whitespace().next())
    else {
        anyhow::bail!("OAuth callback was malformed")
    };
    parse_redirect_target(target, expected_state)
}

pub(crate) fn parse_manual_fallback_input(
    input: &str,
    expected_state: &str,
) -> anyhow::Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        anyhow::bail!("manual OAuth fallback was empty")
    }
    if trimmed.contains('?') || trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let target = trimmed
            .split_once("://")
            .and_then(|(_, rest)| rest.find('/').map(|idx| &rest[idx..]))
            .unwrap_or(trimmed);
        return parse_redirect_target(target, expected_state).map_err(|error| {
            anyhow::anyhow!(
                "manual OAuth fallback rejected: {}",
                redact_oauth_text(&error.to_string())
            )
        });
    }
    if trimmed.contains(char::is_whitespace) || trimmed.contains('&') || trimmed.contains('=') {
        anyhow::bail!(
            "manual OAuth fallback was malformed; paste full redirect URL or authorization code"
        )
    }
    Ok(trimmed.to_string())
}

fn parse_redirect_target(target: &str, expected_state: &str) -> anyhow::Result<String> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if !path.ends_with("/auth/callback") {
        anyhow::bail!("OAuth callback used an unexpected path")
    }
    let params = parse_query(query)?;
    if let Some(error) = params.iter().find(|(k, _)| k == "error").map(|(_, v)| v) {
        anyhow::bail!(
            "OAuth provider rejected login: {}",
            redact_oauth_text(error)
        )
    }
    let state = params
        .iter()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.as_str())
        .unwrap_or_default();
    if state != expected_state {
        anyhow::bail!("OAuth callback state did not match; login was not completed")
    }
    params
        .iter()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.clone())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("OAuth callback did not include authorization code"))
}

pub(crate) fn extract_chatgpt_account_id_from_jwt(access_token: &str) -> anyhow::Result<String> {
    let payload = access_token
        .split('.')
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Codex access token is not a JWT"))?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload)?;
    let value: Value = serde_json::from_slice(&decoded)?;
    value
        .get("https://api.openai.com/auth.chatgpt_account_id")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("https://api.openai.com/auth")
                .and_then(|auth| auth.get("chatgpt_account_id"))
                .and_then(Value::as_str)
        })
        .or_else(|| value.get("chatgpt_account_id").and_then(Value::as_str))
        .or_else(|| value.get("accountId").and_then(Value::as_str))
        .filter(|id| !id.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow::anyhow!("Codex access token is missing ChatGPT account id claim"))
}

pub(crate) fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

pub(crate) fn random_urlsafe(bytes: usize) -> String {
    let mut out = Vec::new();
    while out.len() < bytes {
        out.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    }
    out.truncate(bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(out)
}

pub(crate) fn redact_oauth_text(text: &str) -> String {
    let mut out = String::new();
    for part in text.split('&') {
        if !out.is_empty() {
            out.push('&');
        }
        if part.contains("code=") {
            out.push_str(part.split("code=").next().unwrap_or_default());
            out.push_str("code=<redacted>");
        } else if part.contains("access_token") || part.contains("refresh_token") {
            out.push_str("<redacted>");
        } else {
            out.push_str(part);
        }
    }
    out
}

fn pct(input: &str) -> String {
    let mut out = String::new();
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(b))
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn parse_query(query: &str) -> anyhow::Result<Vec<(String, String)>> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (k, v) = part.split_once('=').unwrap_or((part, ""));
            Ok((decode_pct(k)?, decode_pct(v)?))
        })
        .collect()
}

fn decode_pct(input: &str) -> anyhow::Result<String> {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                anyhow::bail!("OAuth callback query contains malformed percent escape");
            }
            let high = hex_digit(bytes[i + 1]).ok_or_else(|| {
                anyhow::anyhow!("OAuth callback query contains malformed percent escape")
            })?;
            let low = hex_digit(bytes[i + 2]).ok_or_else(|| {
                anyhow::anyhow!("OAuth callback query contains malformed percent escape")
            })?;
            out.push((high << 4) | low);
            i += 3;
        } else {
            out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
            i += 1;
        }
    }
    String::from_utf8(out)
        .map_err(|_| anyhow::anyhow!("OAuth callback query contains invalid UTF-8"))
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_jwt(payload_json: &str) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json);
        format!("{header}.{payload}.")
    }

    #[test]
    fn pkce_verifier_generation_produces_url_safe_string() {
        let verifier = random_urlsafe(64);
        assert!(!verifier.is_empty());
        assert!(
            verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        );
    }

    #[test]
    fn pkce_challenge_matches_sha256_base64_url_no_pad() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn token_response_normalization_extracts_fields() {
        let token = normalize_token_response(TokenResponse {
            access_token: Some(fake_jwt(r#"{"accountId":"acct_123"}"#)),
            refresh_token: Some("refresh".to_string()),
            expires_in: Some(3600),
            account_id_camel: None,
            account_id: None,
            id_token: None,
        })
        .unwrap();

        assert_eq!(token.refresh.as_deref(), Some("refresh"));
        assert_eq!(token.account_id, "acct_123");
        assert!(token.expires.unwrap() > Utc::now().timestamp());
    }

    #[test]
    fn jwt_account_id_extracts_nested_claim() {
        let jwt = fake_jwt(r#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acct_pi"}}"#);
        assert_eq!(
            extract_chatgpt_account_id_from_jwt(&jwt).unwrap(),
            "acct_pi"
        );
    }

    #[test]
    fn authorize_url_contains_required_params() {
        let attempt = OAuthAttempt::from_parts("verifier", "state value");
        let url = attempt.authorization_url();
        assert!(url.starts_with(AUTHORIZE_URL));
        assert!(url.contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"));
        assert!(url.contains("scope=openid%20profile%20email%20offline_access"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=state%20value"));
        assert!(url.contains("id_token_add_organizations=true"));
        assert!(url.contains("codex_cli_simplified_flow=true"));
        assert!(url.contains("originator=pi"));
    }

    #[test]
    fn manual_fallback_parses_redirect_url_and_code() {
        assert_eq!(
            parse_manual_fallback_input(
                "http://localhost:1455/auth/callback?code=manual-code&state=expected",
                "expected"
            )
            .unwrap(),
            "manual-code"
        );
        assert_eq!(
            parse_manual_fallback_input("raw-code", "expected").unwrap(),
            "raw-code"
        );
    }

    #[test]
    fn callback_state_error_redacts_code() {
        let error = parse_callback_request_line(
            "GET /auth/callback?code=secret-code&state=wrong HTTP/1.1",
            "expected",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("state did not match"));
        assert!(!error.contains("secret-code"));
    }
}
