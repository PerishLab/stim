use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "stim",
    version,
    about = "Standalone message downstream for santi"
)]
pub struct Cli {
    #[arg(long, env = "STIM_CONFIG", default_value = "stim.toml")]
    pub config: String,
    #[arg(long = "base-url", env = "STIM_BASE_URL", global = true)]
    pub url: Option<String>,
    #[arg(
        long = "reply-token",
        env = "STIM_REPLY_TOKEN",
        global = true,
        hide_env_values = true
    )]
    pub token: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(subcommand)]
    Service(Service),
    Send {
        text: String,
        #[arg(long = "as", env = "STIM_PARTICIPANT", default_value = "operator")]
        participant: String,
        #[arg(long, env = "STIM_SOUL_ID")]
        soul: Option<String>,
        #[arg(long)]
        request_id: Option<String>,
    },
    Poll {
        #[arg(long = "as", env = "STIM_PARTICIPANT", default_value = "operator")]
        participant: String,
        #[arg(long, default_value_t = 0)]
        since: i64,
    },
    Reply {
        #[arg(conflicts_with_all = ["file", "stdin"], required_unless_present_any = ["file", "stdin"])]
        text: Option<String>,
        #[arg(long, value_name = "PATH", conflicts_with_all = ["text", "stdin"])]
        file: Option<String>,
        #[arg(long, conflicts_with_all = ["text", "file"])]
        stdin: bool,
    },
}

#[derive(Subcommand)]
pub enum Service {
    Serve,
}
