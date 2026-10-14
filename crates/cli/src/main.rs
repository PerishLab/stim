mod cli;
mod http;
mod observation;
mod text;

use anyhow::{Context, Result};
use clap::Parser;

use cli::{Cli, Command, Service};
use stim::{config, server};

#[tokio::main]
async fn main() -> Result<()> {
    plumb::identity!("STIM").map_err(anyhow::Error::msg)?;
    let Cli {
        config,
        url,
        command,
    } = Cli::parse();
    plumb::identity::ready().map_err(anyhow::Error::msg)?;
    match command {
        Command::Service(Service::Serve) => {
            let config = config::Config::load(&config)?;
            server::serve(config).await
        }
        command => observed(url, command).await,
    }
}

#[locus::trace(with = observer.map(observation::Run::view))]
async fn client(
    url: Option<String>,
    command: Command,
    observer: Option<&observation::Run>,
) -> Result<()> {
    let base = url.unwrap_or_else(|| "http://127.0.0.1:43311".to_string());
    let client = reqwest::Client::new();
    match command {
        Command::Send {
            text,
            participant,
            soul,
            request_id,
        } => {
            let request = stim_core::Post {
                participant,
                soul,
                content: text,
                request: request_id
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
            let soul = required("SANTI_SOUL_ID")?;
            let strand = required("SANTI_STRAND_ID")?;
            let turn = required("SANTI_TURN_ID")?;
            let call = required("SANTI_TOOL_CALL_ID")?;
            let effect = required("SANTI_EFFECT_ID")?;
            let capability = required("SANTI_RUNTIME_CAPABILITY")?;
            http::print(
                client
                    .post(format!("{}/api/v1/replies", trim(&base)))
                    .bearer_auth(capability)
                    .json(&stim_core::Reply {
                        soul,
                        strand,
                        turn,
                        call,
                        effect,
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

fn required(name: &str) -> Result<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing {name}"))
}

fn name(command: &Command) -> &'static str {
    match command {
        Command::Send { .. } => "send",
        Command::Poll { .. } => "poll",
        Command::Reply { .. } => "reply",
        Command::Service(_) => unreachable!(),
    }
}

async fn observed(url: Option<String>, command: Command) -> Result<()> {
    let observer = observation::Run::start(name(&command));
    let result = client(url, command, observer.as_ref()).await;
    if let Some(observer) = observer {
        observer.finish(if result.is_ok() { 0 } else { 1 });
    }
    result
}
