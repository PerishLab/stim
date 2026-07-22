use std::io::Read as _;

use anyhow::{Context, Result};

pub fn read(text: Option<String>, file: Option<String>, stdin: bool) -> Result<String> {
    let text = match (text, file, stdin) {
        (Some(text), None, false) => text,
        (None, Some(path), false) if path == "-" => stdin_text()?,
        (None, Some(path), false) => {
            std::fs::read_to_string(&path).with_context(|| format!("read {path}"))?
        }
        (None, None, true) => stdin_text()?,
        _ => anyhow::bail!("provide exactly one reply text source"),
    };
    if text.trim().is_empty() {
        anyhow::bail!("reply text must not be empty");
    }
    Ok(text)
}

fn stdin_text() -> Result<String> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .context("read stdin")?;
    Ok(text)
}
