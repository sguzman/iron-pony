use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use iron_pony_core::{
    FortuneConfig, Mode, RenderConfig, default_balloon_paths, default_pony_paths, list_ponies,
    pick_fortune, render, select_pony,
};
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "iron-pony",
    version,
    about = "Rust port baseline for ponysay with parity harness support"
)]
struct Cli {
    #[arg(short = 'f', long = "pony", help = "Pony template name or path")]
    pony: Option<String>,

    #[arg(short = 'b', long = "balloon", help = "Balloon style name")]
    balloon: Option<String>,

    #[arg(long = "think", help = "Render using think mode")]
    think: bool,

    #[arg(long = "wrap", default_value_t = 40, help = "Balloon wrap width")]
    wrap: usize,

    #[arg(
        long = "ponydir",
        value_delimiter = ':',
        help = "Pony search path override"
    )]
    pony_paths: Vec<PathBuf>,

    #[arg(
        long = "balloondir",
        value_delimiter = ':',
        help = "Balloon search path override"
    )]
    balloon_paths: Vec<PathBuf>,

    #[arg(long = "list", help = "List available ponies")]
    list: bool,

    #[arg(long = "fortune", help = "Use internal fortune selection")]
    fortune: bool,

    #[arg(
        long = "fortune-all",
        help = "Include offensive fortunes in internal fortune mode"
    )]
    fortune_all: bool,

    #[arg(
        long = "fortune-equal",
        help = "Select fortune files uniformly in internal fortune mode"
    )]
    fortune_equal: bool,

    #[arg(
        long = "fortune-path",
        value_delimiter = ':',
        help = "Fortune database search path override"
    )]
    fortune_paths: Vec<PathBuf>,

    #[arg(long = "seed", help = "Deterministic seed for random selection")]
    seed: Option<u64>,

    #[arg(value_name = "MESSAGE", trailing_var_arg = true)]
    message: Vec<String>,
}

fn main() -> ExitCode {
    init_tracing();

    let cli = Cli::parse();
    debug!(?cli, "parsed CLI options");

    let pony_paths = if cli.pony_paths.is_empty() {
        env_paths("PONYSAY_PONY_PATH").unwrap_or_else(default_pony_paths)
    } else {
        cli.pony_paths.clone()
    };

    let balloon_paths = if cli.balloon_paths.is_empty() {
        env_paths("PONYSAY_BALLOON_PATH").unwrap_or_else(default_balloon_paths)
    } else {
        cli.balloon_paths.clone()
    };

    if cli.list {
        let names = list_ponies(&pony_paths);
        for name in names {
            println!("{name}");
        }
        return ExitCode::SUCCESS;
    }

    let message = match resolve_message(&cli) {
        Ok(message) => message,
        Err(error) => {
            error!(%error, "failed to resolve message input");
            eprintln!("iron-pony: {error}");
            return ExitCode::from(1);
        }
    };

    let pony = match select_pony(cli.pony.as_deref(), &pony_paths, cli.seed) {
        Ok(pony) => pony,
        Err(error) => {
            error!(%error, "failed to resolve pony");
            eprintln!("iron-pony: {error}");
            return ExitCode::from(1);
        }
    };

    let config = RenderConfig {
        message,
        pony,
        pony_paths,
        balloon: cli.balloon,
        balloon_paths,
        mode: if cli.think { Mode::Think } else { Mode::Say },
        wrap_width: cli.wrap.max(1),
    };

    match render(&config) {
        Ok(output) => {
            info!("render completed");
            let mut stdout = io::stdout().lock();
            if let Err(error) = writeln!(stdout, "{output}") {
                error!(%error, "failed to write output");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            error!(%error, "render failed");
            eprintln!("iron-pony: {error}");
            ExitCode::from(1)
        }
    }
}

fn resolve_message(cli: &Cli) -> Result<String, String> {
    if cli.fortune {
        info!("using internal fortune mode");
        let mut fortune_config = FortuneConfig::default();
        fortune_config.include_offensive = cli.fortune_all;
        fortune_config.equal_files = cli.fortune_equal;
        fortune_config.seed = cli.seed;
        if !cli.fortune_paths.is_empty() {
            fortune_config.search_paths = cli.fortune_paths.clone();
        } else if let Some(paths) = env_paths("FORTUNE_PATH") {
            fortune_config.search_paths = paths;
        }
        return pick_fortune(&fortune_config).map_err(|error| error.to_string());
    }

    if !cli.message.is_empty() {
        return Ok(cli.message.join(" "));
    }

    let mut stdin = io::stdin();
    if !stdin.is_terminal() {
        let mut data = String::new();
        stdin
            .read_to_string(&mut data)
            .map_err(|error| format!("failed reading stdin: {error}"))?;
        let trimmed = data.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    warn!("no message source resolved");
    Err("no message provided".to_string())
}

fn env_paths(var: &str) -> Option<Vec<PathBuf>> {
    let value = std::env::var(var).ok()?;
    let mut paths = Vec::new();
    for part in value.split(':') {
        if part.is_empty() {
            continue;
        }
        paths.push(PathBuf::from(part));
    }
    if paths.is_empty() { None } else { Some(paths) }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "info,iron_pony_core=debug,iron_pony_cli=debug,iron_pony_parity=debug,xtask=debug",
        )
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_ansi(true)
        .init();
}
