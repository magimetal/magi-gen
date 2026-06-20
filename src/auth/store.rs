use crate::{auth::codex, config::AppPaths};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, fs, path::Path};

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Auth {
    #[serde(default)]
    pub providers: BTreeMap<String, AuthProviderRecord>,
}

impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Auth")
            .field("providers", &self.providers)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum AuthProviderRecord {
    #[serde(rename = "oauth")]
    OAuth {
        access: String,
        #[serde(default)]
        refresh: Option<String>,
        #[serde(default)]
        expires: Option<i64>,
        #[serde(default)]
        account_id: Option<String>,
    },
}

impl fmt::Debug for AuthProviderRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OAuth { expires, .. } => f
                .debug_struct("OAuth")
                .field("access", &"<redacted>")
                .field("refresh", &"<redacted>")
                .field("expires", expires)
                .field("account_id", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexCredential {
    pub access: String,
    pub account_id: String,
}

pub fn read_auth(paths: &AppPaths) -> anyhow::Result<Auth> {
    validate_auth_file_before_read(&paths.auth_file)?;
    if !paths.auth_file.exists() {
        return Ok(Auth::default());
    }
    let text = fs::read_to_string(&paths.auth_file)
        .with_context(|| format!("could not read {}", paths.auth_file.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("could not parse {}", paths.auth_file.display()))
}

pub fn write_auth(paths: &AppPaths, auth: &Auth) -> anyhow::Result<()> {
    paths.ensure_dirs()?;
    atomic_write_json(&paths.auth_file, auth, Some(0o600))
}

pub fn update_auth(paths: &AppPaths, mutate: impl FnOnce(&mut Auth)) -> anyhow::Result<Auth> {
    let mut auth = read_auth(paths)?;
    mutate(&mut auth);
    write_auth(paths, &auth)?;
    Ok(auth)
}

pub fn logout_provider(paths: &AppPaths, provider: &str) -> anyhow::Result<bool> {
    let mut auth = read_auth(paths)?;
    let removed = auth.providers.remove(provider).is_some();
    if removed {
        write_auth(paths, &auth)?;
    }
    Ok(removed)
}

pub fn codex_credential(paths: &AppPaths) -> anyhow::Result<CodexCredential> {
    codex_credential_with_refresh(paths, codex::refresh_token)
}

pub(crate) fn codex_credential_with_refresh(
    paths: &AppPaths,
    refresh_exchange: impl FnOnce(&str) -> anyhow::Result<codex::NormalizedToken>,
) -> anyhow::Result<CodexCredential> {
    let auth = read_auth(paths)?;
    let Some(record) = auth.providers.get(crate::providers::CODEX_PROVIDER) else {
        anyhow::bail!("Not logged in. Run: magi-image-gen-cli login codex")
    };
    let AuthProviderRecord::OAuth {
        access,
        refresh,
        expires,
        account_id,
    } = record;
    if access.is_empty() {
        anyhow::bail!("Not logged in. Run: magi-image-gen-cli login codex")
    }
    if token_needs_refresh(*expires) {
        let refresh = refresh
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("Codex OAuth expired. Run: magi-image-gen-cli login codex")
            })?;
        let token = refresh_exchange(refresh).map_err(|_| {
            anyhow::anyhow!("Codex OAuth expired. Run: magi-image-gen-cli login codex")
        })?;
        let credential = CodexCredential {
            access: token.access.clone(),
            account_id: token.account_id.clone(),
        };
        codex::persist_token(paths, token)?;
        return Ok(credential);
    }
    let account_id = account_id
        .clone()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Codex OAuth record missing account id. Run: magi-image-gen-cli login codex"
            )
        })?;
    Ok(CodexCredential {
        access: access.clone(),
        account_id,
    })
}

pub(crate) fn token_needs_refresh(expires: Option<i64>) -> bool {
    expires.is_none_or(|value| value <= chrono::Utc::now().timestamp() + codex::REFRESH_SKEW_SECS)
}

pub fn validate_auth_file_before_read(path: &Path) -> anyhow::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() {
        anyhow::bail!(
            "auth.json must be a regular private file; symlinked auth files are not allowed"
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            anyhow::bail!("auth.json permissions must be private/owner-only (0600 or stricter)");
        }
    }
    Ok(())
}

pub fn atomic_write_json<T: Serialize>(
    path: &Path,
    value: &T,
    mode: Option<u32>,
) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    atomic_write_bytes(path, &bytes, mode)
}

pub fn atomic_write_bytes(path: &Path, bytes: &[u8], mode: Option<u32>) -> anyhow::Result<()> {
    let parent = path.parent().context("path has no parent")?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file"),
        uuid::Uuid::new_v4()
    ));
    fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn auth_rejects_world_readable_file() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("auth.json");
        fs::write(&path, "{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        let error = validate_auth_file_before_read(&path)
            .unwrap_err()
            .to_string();

        assert!(error.contains("permissions"), "{error}");
    }

    #[test]
    #[cfg(unix)]
    fn auth_rejects_symlink() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::TempDir::new().unwrap();
        let target = temp.path().join("target.json");
        let link = temp.path().join("auth.json");
        fs::write(&target, "{}").unwrap();
        symlink(&target, &link).unwrap();

        let error = validate_auth_file_before_read(&link)
            .unwrap_err()
            .to_string();

        assert!(error.contains("symlink"), "{error}");
    }

    #[test]
    #[cfg(unix)]
    fn auth_write_uses_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::TempDir::new().unwrap();
        let paths = AppPaths::from_root(temp.path().join("app"));
        write_auth(&paths, &Auth::default()).unwrap();

        let mode = fs::metadata(&paths.auth_file).unwrap().permissions().mode() & 0o777;

        assert_eq!(mode, 0o600);
    }

    #[test]
    fn refresh_skew_detects_expiring_token() {
        let now = chrono::Utc::now().timestamp();
        assert!(token_needs_refresh(Some(
            now + codex::REFRESH_SKEW_SECS - 1
        )));
        assert!(!token_needs_refresh(Some(
            now + codex::REFRESH_SKEW_SECS + 60
        )));
    }

    #[test]
    fn codex_credential_refreshes_and_persists_near_expiry_token() {
        let temp = tempfile::TempDir::new().unwrap();
        let paths = AppPaths::from_root(temp.path().join("app"));
        let mut auth = Auth::default();
        auth.providers.insert(
            crate::providers::CODEX_PROVIDER.to_string(),
            AuthProviderRecord::OAuth {
                access: "old-access".to_string(),
                refresh: Some("old-refresh".to_string()),
                expires: Some(chrono::Utc::now().timestamp()),
                account_id: Some("old-acct".to_string()),
            },
        );
        write_auth(&paths, &auth).unwrap();

        let credential = codex_credential_with_refresh(&paths, |refresh| {
            assert_eq!(refresh, "old-refresh");
            Ok(codex::NormalizedToken {
                access: "new-access".to_string(),
                refresh: None,
                expires: Some(chrono::Utc::now().timestamp() + 3600),
                account_id: "new-acct".to_string(),
            })
        })
        .unwrap();

        assert_eq!(credential.access, "new-access");
        assert_eq!(credential.account_id, "new-acct");
        let auth = read_auth(&paths).unwrap();
        let Some(AuthProviderRecord::OAuth {
            access,
            refresh,
            account_id,
            ..
        }) = auth.providers.get(crate::providers::CODEX_PROVIDER)
        else {
            panic!("missing codex record")
        };
        assert_eq!(access, "new-access");
        assert_eq!(refresh.as_deref(), Some("old-refresh"));
        assert_eq!(account_id.as_deref(), Some("new-acct"));
    }

    #[test]
    fn logout_removes_provider_record() {
        let temp = tempfile::TempDir::new().unwrap();
        let paths = AppPaths::from_root(temp.path().join("app"));
        let mut auth = Auth::default();
        auth.providers.insert(
            crate::providers::CODEX_PROVIDER.to_string(),
            AuthProviderRecord::OAuth {
                access: "access".to_string(),
                refresh: Some("refresh".to_string()),
                expires: Some(1),
                account_id: Some("acct".to_string()),
            },
        );
        write_auth(&paths, &auth).unwrap();

        assert!(logout_provider(&paths, crate::providers::CODEX_PROVIDER).unwrap());
        assert!(
            !read_auth(&paths)
                .unwrap()
                .providers
                .contains_key(crate::providers::CODEX_PROVIDER)
        );
    }
}
