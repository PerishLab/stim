use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "stim",
    version = plumb::version!("STIM"),
    about = "Standalone message downstream for santi",
    after_help = "Finite CLI observation is off by default. Set [locus] enabled = true in config.toml under ~/.stim on Unix or LOCALAPPDATA/stim on Windows, separate from STIM_CONFIG and STIM_BASE_URL. Buffer is state/locus; endpoint overrides LOCUS_API, then http://127.0.0.1:43308. Observation preserves requests, results and exit status. service serve does not initialize observation."
)]
pub struct Cli {
    #[arg(long, env = "STIM_CONFIG", default_value = "stim.toml")]
    pub config: String,
    #[arg(long = "base-url", env = "STIM_BASE_URL", global = true)]
    pub url: Option<String>,
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
        #[arg(long = "request-id", value_name = "REQUEST_ID")]
        request: Option<String>,
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
