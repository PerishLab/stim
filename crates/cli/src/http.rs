use anyhow::{Context, Result};

pub async fn print(response: reqwest::Response) -> Result<()> {
    let status = response.status();
    let text = response.text().await.context("read response")?;
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value)?),
        Err(_) => println!("{text}"),
    }
    if !status.is_success() {
        anyhow::bail!("request failed with status {status}");
    }
    Ok(())
}

pub fn segment(value: &str) -> String {
    let mut result = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            byte => result.push_str(&format!("%{byte:02X}")),
        }
    }
    result
}
