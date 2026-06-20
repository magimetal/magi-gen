use crate::auth::store::{self, Auth};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{env, fs, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub root: PathBuf,
    pub auth_file: PathBuf,
    pub settings_file: PathBuf,
    pub cache_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> anyhow::Result<Self> {
        if let Some(root) = env::var_os("MAGI_IMAGE_GEN_HOME") {
            return Ok(Self::from_root(PathBuf::from(root)));
        }
        let home = dirs::home_dir().context("could not resolve home directory")?;
        Ok(Self::from_root(home.join(".magi-image-gen-cli")))
    }

    pub fn from_root(root: PathBuf) -> Self {
        Self {
            auth_file: root.join("auth.json"),
            settings_file: root.join("settings.json"),
            cache_dir: root.join("cache"),
            root,
        }
    }

    pub fn ensure_dirs(&self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(&self.cache_dir)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    pub default_provider: String,
    pub codex: CodexSettings,
    pub openai_compatible: OpenAiCompatibleSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexSettings {
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenAiCompatibleSettings {
    pub base_url: String,
    pub api_key_env_var: String,
    pub model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_provider: "codex".to_string(),
            codex: CodexSettings {
                model: crate::providers::DEFAULT_CODEX_MODEL.to_string(),
            },
            openai_compatible: OpenAiCompatibleSettings {
                base_url: "https://api.openai.com/v1".to_string(),
                api_key_env_var: "OPENAI_API_KEY".to_string(),
                model: crate::providers::DEFAULT_CODEX_MODEL.to_string(),
            },
        }
    }
}

pub fn read_settings(paths: &AppPaths) -> anyhow::Result<Settings> {
    if !paths.settings_file.exists() {
        return Ok(Settings::default());
    }
    let text = fs::read_to_string(&paths.settings_file)
        .with_context(|| format!("could not read {}", paths.settings_file.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("could not parse {}", paths.settings_file.display()))
}

pub fn write_settings(paths: &AppPaths, settings: &Settings) -> anyhow::Result<()> {
    paths.ensure_dirs()?;
    store::atomic_write_json(&paths.settings_file, settings, Some(0o600))
}

pub fn read_auth(paths: &AppPaths) -> anyhow::Result<Auth> {
    store::read_auth(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let temp = tempfile::TempDir::new().unwrap();
        let paths = AppPaths::from_root(temp.path().join("app"));
        let settings = Settings {
            default_provider: "codex".to_string(),
            codex: CodexSettings {
                model: "gpt-5.4".to_string(),
            },
            openai_compatible: OpenAiCompatibleSettings {
                base_url: "https://example.test/v1".to_string(),
                api_key_env_var: "EXAMPLE_KEY".to_string(),
                model: "gpt-5.4".to_string(),
            },
        };

        write_settings(&paths, &settings).unwrap();
        assert_eq!(read_settings(&paths).unwrap(), settings);
    }
}
