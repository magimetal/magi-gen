use anyhow::Context;
use base64::Engine;
use std::{fs, path::Path};

pub fn write_image_result(base64_image: &str, output: &Path) -> anyhow::Result<()> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_image)
        .context("image result was not valid base64")?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, bytes).with_context(|| format!("could not write {}", output.display()))
}

pub fn print_base64(base64_image: &str) {
    println!("{base64_image}");
}
