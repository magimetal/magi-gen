use crate::{
    auth::store::{self, AuthProviderRecord},
    config::AppPaths,
    providers::CODEX_PROVIDER,
};
use anyhow::Context;
use std::path::{Path, PathBuf};

const MAGI_CODE_CODEX_PROVIDER: &str = "openai-codex";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportResult {
    pub overwritten: bool,
}

pub fn import_magi_code(paths: &AppPaths) -> anyhow::Result<ImportResult> {
    let source = default_magi_code_auth_file()?;
    import_magi_code_from_file(paths, &source)
}

fn default_magi_code_auth_file() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().context("could not resolve home directory")?;
    Ok(home.join(".mc").join("auth.json"))
}

pub fn import_magi_code_from_file(paths: &AppPaths, source: &Path) -> anyhow::Result<ImportResult> {
    if !source.exists() {
        anyhow::bail!("magi-code auth file not found at {}", source.display())
    }
    let source_paths = AppPaths::from_root(
        source
            .parent()
            .context("magi-code auth path has no parent")?
            .to_path_buf(),
    );
    let source_auth = store::read_auth(&source_paths)
        .with_context(|| format!("could not read magi-code auth file at {}", source.display()))?;
    let record = source_auth
        .providers
        .get(MAGI_CODE_CODEX_PROVIDER)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("magi-code auth has no openai-codex OAuth record"))?;
    if !matches!(record, AuthProviderRecord::OAuth { .. }) {
        anyhow::bail!("magi-code openai-codex record is not OAuth")
    }

    let mut target_auth = store::read_auth(paths)?;
    let overwritten = target_auth.providers.contains_key(CODEX_PROVIDER);
    target_auth
        .providers
        .insert(CODEX_PROVIDER.to_string(), record);
    store::write_auth(paths, &target_auth)?;
    Ok(ImportResult { overwritten })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::store::Auth;

    fn oauth(access: &str) -> AuthProviderRecord {
        AuthProviderRecord::OAuth {
            access: access.to_string(),
            refresh: Some(format!("{access}-refresh")),
            expires: Some(1234567890),
            account_id: Some(format!("{access}-acct")),
        }
    }

    #[test]
    fn import_magi_code_maps_openai_codex_to_codex() {
        let temp = tempfile::TempDir::new().unwrap();
        let mc_paths = AppPaths::from_root(temp.path().join(".mc"));
        let app_paths = AppPaths::from_root(temp.path().join("app"));
        let mut auth = Auth::default();
        auth.providers
            .insert(MAGI_CODE_CODEX_PROVIDER.to_string(), oauth("imported"));
        store::write_auth(&mc_paths, &auth).unwrap();

        let result = import_magi_code_from_file(&app_paths, &mc_paths.auth_file).unwrap();

        assert!(!result.overwritten);
        let app_auth = store::read_auth(&app_paths).unwrap();
        assert_eq!(
            app_auth.providers.get(CODEX_PROVIDER),
            Some(&oauth("imported"))
        );
        assert!(!app_auth.providers.contains_key(MAGI_CODE_CODEX_PROVIDER));
    }

    #[test]
    fn import_magi_code_missing_file_returns_clear_error() {
        let temp = tempfile::TempDir::new().unwrap();
        let app_paths = AppPaths::from_root(temp.path().join("app"));
        let missing = temp.path().join(".mc").join("auth.json");

        let error = import_magi_code_from_file(&app_paths, &missing)
            .unwrap_err()
            .to_string();

        assert!(error.contains("magi-code auth file not found"), "{error}");
    }

    #[test]
    fn import_magi_code_no_codex_record_returns_clear_error() {
        let temp = tempfile::TempDir::new().unwrap();
        let mc_paths = AppPaths::from_root(temp.path().join(".mc"));
        let app_paths = AppPaths::from_root(temp.path().join("app"));
        store::write_auth(&mc_paths, &Auth::default()).unwrap();

        let error = import_magi_code_from_file(&app_paths, &mc_paths.auth_file)
            .unwrap_err()
            .to_string();

        assert!(error.contains("no openai-codex OAuth record"), "{error}");
    }

    #[test]
    fn import_magi_code_overwrites_existing_codex_record() {
        let temp = tempfile::TempDir::new().unwrap();
        let mc_paths = AppPaths::from_root(temp.path().join(".mc"));
        let app_paths = AppPaths::from_root(temp.path().join("app"));
        let mut source_auth = Auth::default();
        source_auth
            .providers
            .insert(MAGI_CODE_CODEX_PROVIDER.to_string(), oauth("new"));
        store::write_auth(&mc_paths, &source_auth).unwrap();
        let mut target_auth = Auth::default();
        target_auth
            .providers
            .insert(CODEX_PROVIDER.to_string(), oauth("old"));
        store::write_auth(&app_paths, &target_auth).unwrap();

        let result = import_magi_code_from_file(&app_paths, &mc_paths.auth_file).unwrap();

        assert!(result.overwritten);
        let app_auth = store::read_auth(&app_paths).unwrap();
        assert_eq!(app_auth.providers.get(CODEX_PROVIDER), Some(&oauth("new")));
    }
}
