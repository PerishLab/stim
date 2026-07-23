mod cli;
mod http;
mod text;

use anyhow::{Context, Result};
use clap::Parser;

use cli::{Cli, Command, ServiceCommand};
use stim::{config, server};

#[tokio::main]
async fn main() -> Result<()> {
    let Cli {
        config,
        base_url,
        reply_token,
        command,
    } = Cli::parse();
    match command {
        Command::Service(ServiceCommand::Serve) => {
            let config = config::Config::load(&config)?;
            server::serve(config).await
        }
        command => client(base_url, reply_token, command).await,
    }
}

async fn client(
    base_url: Option<String>,
    reply_token: Option<String>,
    command: Command,
) -> Result<()> {
    let base = base_url.unwrap_or_else(|| "http://127.0.0.1:43308".to_string());
    let client = reqwest::Client::new();
    match command {
        Command::Send {
            text,
            participant,
            soul,
            request_id,
        } => {
            let request = stim_core::MessageRequest {
                participant_id: participant,
                soul_id: soul,
                content: text,
                request_id: request_id
                    .unwrap_or_else(|| format!("req_{}", uuid::Uuid::new_v4().simple())),
            };
            http::print(
                client
                    .post(format!("{}/api/v1/messages", trim(&base)))
                    .json(&request)
                    .send()
                    .await
                    .context("send message")?,
            )
            .await
        }
        Command::Poll { participant, since } => {
            http::print(
                client
                    .get(format!(
                        "{}/api/v1/inbox/{}",
                        trim(&base),
                        http::segment(&participant)
                    ))
                    .query(&[("since", since)])
                    .send()
                    .await
                    .context("poll inbox")?,
            )
            .await
        }
        Command::Reply { text, file, stdin } => {
            let content = text::read(text, file, stdin)?;
            let strand_id = required_env("SANTI_STRAND_ID")?;
            let turn_id = required_env("SANTI_TURN_ID")?;
            let token = reply_token
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow::anyhow!("missing --reply-token / STIM_REPLY_TOKEN"))?;
            http::print(
                client
                    .post(format!("{}/api/v1/replies", trim(&base)))
                    .bearer_auth(token)
                    .json(&stim_core::ReplyRequest {
                        strand_id,
                        turn_id,
                        content,
                    })
                    .send()
                    .await
                    .context("send early reply")?,
            )
            .await
        }
        Command::Service(_) => unreachable!(),
    }
}

fn trim(value: &str) -> &str {
    value.trim_end_matches('/')
}

fn required_env(name: &str) -> Result<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing {name}"))
}
