mod auth;
mod chromakey;
mod cli;
mod config;
mod import;
mod output;
mod providers;

use crate::{
    auth::store::{self, AuthProviderRecord},
    cli::{AuthCommand, Cli, Command, GenerateArgs, ProviderKind},
    config::AppPaths,
    providers::{
        ImageProvider, codex::CodexImageProvider, openai_compatible::OpenAiCompatibleImageProvider,
        request::ImageRequest,
    },
};
use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Some(Command::Generate(args)) => generate(args),
        Some(Command::Login { provider }) => match provider {
            cli::LoginProvider::Codex => login_codex(),
        },
        Some(Command::Logout { provider }) => match provider {
            cli::LoginProvider::Codex => logout_codex(),
        },
        Some(Command::Auth { command }) => handle_auth(command),
        Some(Command::Import { source }) => match source {
            cli::ImportSource::MagiCode => import_magi_code(),
        },
        None => {
            let prompt = cli.prompt.context("missing prompt or subcommand")?;
            generate(GenerateArgs {
                prompt,
                provider: ProviderKind::Codex,
                output: cli.output,
                model: providers::DEFAULT_CODEX_MODEL.to_string(),
                size: "1024x1024".to_string(),
                quality: "low".to_string(),
                output_format: cli.output_format,
                base_url: None,
                api_key_env: None,
                base64: cli.base64,
                transparent: cli.transparent,
            })
        }
    }
}

fn generate(args: GenerateArgs) -> anyhow::Result<()> {
    let paths = AppPaths::resolve()?;
    let settings = config::read_settings(&paths)?;
    if !paths.settings_file.exists() {
        config::write_settings(&paths, &settings)?;
    }
    match args.provider {
        ProviderKind::Codex => generate_codex(args, &paths),
        ProviderKind::OpenaiCompatible => generate_openai_compatible(args),
    }
}

fn generate_codex(args: GenerateArgs, paths: &AppPaths) -> anyhow::Result<()> {
    if args.model == "gpt-image-2" {
        anyhow::bail!("Codex provider does not support gpt-image-2 as Responses model; use gpt-5.5")
    }
    let credential = store::codex_credential(paths)?;
    let provider = CodexImageProvider::new(credential.access, credential.account_id)?;
    let prompt = args.prompt.clone();
    let output_format = effective_output_format(&args.output_format, args.transparent);
    let result = provider.generate(ImageRequest {
        prompt: args.prompt,
        model: args.model,
        size: args.size,
        quality: args.quality,
        output_format: output_format.clone(),
        transparent: args.transparent,
    })?;
    write_generate_result(
        result,
        args.output,
        args.base64,
        &prompt,
        &output_format,
        args.transparent,
    )
}

fn generate_openai_compatible(args: GenerateArgs) -> anyhow::Result<()> {
    let base_url = require_openai_compatible_base_url(args.base_url)?;
    let api_key_env = args
        .api_key_env
        .unwrap_or_else(|| "OPENAI_API_KEY".to_string());
    let api_key = openai_compatible_api_key_from_env(&api_key_env)?;
    let provider = OpenAiCompatibleImageProvider::new(api_key, base_url)?;
    let prompt = args.prompt.clone();
    let output_format = effective_output_format(&args.output_format, args.transparent);
    let result = provider.generate(ImageRequest {
        prompt: args.prompt,
        model: args.model,
        size: args.size,
        quality: args.quality,
        output_format: output_format.clone(),
        transparent: args.transparent,
    })?;
    write_generate_result(
        result,
        args.output,
        args.base64,
        &prompt,
        &output_format,
        args.transparent,
    )
}

fn write_generate_result(
    result: providers::request::ImageResult,
    output: Option<PathBuf>,
    base64: bool,
    prompt: &str,
    output_format: &str,
    transparent: bool,
) -> anyhow::Result<()> {
    let original_base64 = result.base64.clone();
    let (base64_image, detected_color) = if transparent {
        let (base64_image, color) = chromakey::transparent_png_base64(&result.base64)?;
        (base64_image, Some(color))
    } else {
        (result.base64, None)
    };

    if let Some(color) = detected_color {
        eprintln!(
            "detected transparent background color #{:02X}{:02X}{:02X}",
            color.r, color.g, color.b
        );
    }

    if base64 {
        output::print_base64(&base64_image);
    } else {
        let output = output.unwrap_or_else(|| default_output_for_prompt(prompt, output_format));
        output::write_image_result(&base64_image, &output)?;
        eprintln!("wrote {}", output.display());

        if transparent {
            let mut original_path = output.clone();
            if let Some(stem) = output.file_stem() {
                let mut name = stem.to_os_string();
                name.push("-original");
                original_path.set_file_name(name);
                if let Some(ext) = output.extension() {
                    original_path.set_extension(ext);
                }
            }
            output::write_image_result(&original_base64, &original_path)?;
            eprintln!("wrote {}", original_path.display());
        }
    }
    Ok(())
}

fn effective_output_format(output_format: &str, transparent: bool) -> String {
    if transparent {
        if output_format != "png" {
            eprintln!(
                "warning: --transparent requires PNG output; ignoring --output-format {output_format}"
            );
        }
        "png".to_string()
    } else {
        output_format.to_string()
    }
}

fn default_output_for_prompt(prompt: &str, output_format: &str) -> PathBuf {
    PathBuf::from(format!("{}.{}", slugify_prompt(prompt), output_format))
}

fn slugify_prompt(prompt: &str) -> String {
    let mut slug = String::new();
    let mut last_was_hyphen = false;
    for ch in prompt.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            if slug.len() >= 48 {
                break;
            }
            slug.push(ch);
            last_was_hyphen = false;
        } else if !last_was_hyphen && !slug.is_empty() && slug.len() < 48 {
            slug.push('-');
            last_was_hyphen = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "image".to_string()
    } else {
        slug
    }
}

fn require_openai_compatible_base_url(base_url: Option<String>) -> anyhow::Result<String> {
    base_url.context("--base-url is required for openai-compatible provider")
}

fn openai_compatible_api_key_from_env(var_name: &str) -> anyhow::Result<String> {
    std::env::var(var_name).map_err(|_| anyhow::anyhow!("Missing API key env var {var_name}"))
}

fn login_codex() -> anyhow::Result<()> {
    let paths = AppPaths::resolve()?;
    let result = auth::codex::login_codex(&paths)?;
    println!("{}", result.message);
    Ok(())
}

fn logout_codex() -> anyhow::Result<()> {
    let paths = AppPaths::resolve()?;
    let removed = store::logout_provider(&paths, providers::CODEX_PROVIDER)?;
    if removed {
        println!("codex: removed");
    } else {
        println!("codex: missing");
    }
    Ok(())
}

fn handle_auth(command: AuthCommand) -> anyhow::Result<()> {
    match command {
        AuthCommand::Status => {
            let paths = AppPaths::resolve()?;
            let auth = config::read_auth(&paths)?;
            if matches!(
                auth.providers.get(providers::CODEX_PROVIDER),
                Some(AuthProviderRecord::OAuth { .. })
            ) {
                println!("codex: configured");
            } else {
                println!("codex: missing");
            }
            Ok(())
        }
    }
}

fn import_magi_code() -> anyhow::Result<()> {
    let paths = AppPaths::resolve()?;
    let result = import::import_magi_code(&paths)?;
    if result.overwritten {
        eprintln!("warning: overwrote existing codex auth record");
    }
    println!("codex: imported from magi-code");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_openai_compatible_base_url_error_is_clear() {
        let error = require_openai_compatible_base_url(None)
            .unwrap_err()
            .to_string();

        assert_eq!(
            error,
            "--base-url is required for openai-compatible provider"
        );
    }

    #[test]
    fn missing_openai_compatible_api_key_env_error_is_clear() {
        let error = openai_compatible_api_key_from_env("MAGI_IMAGE_GEN_TEST_MISSING_KEY")
            .unwrap_err()
            .to_string();

        assert_eq!(
            error,
            "Missing API key env var MAGI_IMAGE_GEN_TEST_MISSING_KEY"
        );
    }

    #[test]
    fn filename_derivation_slugifies_prompt() {
        assert_eq!(
            default_output_for_prompt("Cyberpunk raccoon eating ramen!!!", "png"),
            PathBuf::from("cyberpunk-raccoon-eating-ramen.png")
        );
        assert_eq!(
            default_output_for_prompt("Cyberpunk raccoon eating ramen!!!", "webp"),
            PathBuf::from("cyberpunk-raccoon-eating-ramen.webp")
        );
        assert_eq!(
            default_output_for_prompt("Cyberpunk raccoon eating ramen!!!", "jpeg"),
            PathBuf::from("cyberpunk-raccoon-eating-ramen.jpeg")
        );
        assert_eq!(
            default_output_for_prompt("***", "png"),
            PathBuf::from("image.png")
        );
        assert_eq!(
            slugify_prompt("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"),
            "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuv"
        );
    }

    #[test]
    fn base64_output_does_not_require_output_path() {
        write_generate_result(
            providers::request::ImageResult {
                base64: "abc123".to_string(),
                mime_hint: Some("image/png".to_string()),
            },
            None,
            true,
            "prompt",
            "png",
            false,
        )
        .unwrap();
    }

    #[test]
    fn transparent_output_format_is_forced_to_png() {
        assert_eq!(effective_output_format("webp", true), "png");
        assert_eq!(effective_output_format("jpeg", true), "png");
        assert_eq!(effective_output_format("png", true), "png");
        assert_eq!(effective_output_format("webp", false), "webp");
    }
}
