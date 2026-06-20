use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "magi-image-gen-cli",
    version,
    about = "Generate images with Codex-backed Responses API"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Prompt shorthand when no subcommand is provided.
    pub prompt: Option<String>,

    #[arg(short, long)]
    pub output: Option<PathBuf>,

    #[arg(long)]
    pub base64: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Generate(GenerateArgs),
    Login {
        provider: LoginProvider,
    },
    Logout {
        provider: LoginProvider,
    },
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    Import {
        source: ImportSource,
    },
}

#[derive(Debug, Args, Clone)]
pub struct GenerateArgs {
    pub prompt: String,
    #[arg(long, default_value = "codex")]
    pub provider: ProviderKind,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long, default_value = "gpt-5.5")]
    pub model: String,
    #[arg(long, default_value = "1024x1024")]
    pub size: String,
    #[arg(long, default_value = "low")]
    pub quality: String,
    #[arg(long)]
    pub base_url: Option<String>,
    #[arg(long)]
    pub api_key_env: Option<String>,
    #[arg(long)]
    pub base64: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProviderKind {
    Codex,
    OpenaiCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LoginProvider {
    Codex,
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ImportSource {
    MagiCode,
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
