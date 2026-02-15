use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use iron_pony_parity::{ParityConfig, run_parity};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "xtask", version, about = "Project automation tasks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Parity {
        #[arg(long, default_value = "tests/parity_cases")]
        cases: PathBuf,
        #[arg(long, default_value = "spec/requirements.yaml")]
        spec: PathBuf,
        #[arg(long, default_value = "target/parity")]
        out: PathBuf,
        #[arg(long, default_value = "ponysay")]
        reference: String,
        #[arg(long)]
        candidate: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    match cli.command {
        Command::Parity {
            cases,
            spec,
            out,
            reference,
            candidate,
        } => run_parity_task(cases, spec, out, reference, candidate),
    }
}

fn run_parity_task(
    cases: PathBuf,
    spec: PathBuf,
    out: PathBuf,
    reference: String,
    candidate: Option<PathBuf>,
) -> Result<()> {
    let workspace_root = std::env::current_dir().context("failed to resolve current dir")?;
    let config = ParityConfig {
        workspace_root: workspace_root.clone(),
        cases_dir: workspace_root.join(cases),
        spec_path: workspace_root.join(spec),
        output_dir: workspace_root.join(out),
        reference_program: reference,
        candidate_program: candidate,
    };

    let report = run_parity(&config)?;
    info!(
        case_parity = report.summary.case_parity,
        requirement_parity = report.summary.weighted_requirement_parity,
        "parity run completed"
    );

    println!("Parity report written to {}", config.output_dir.display());
    println!(
        "case parity: {:.2}% | weighted requirement parity: {:.2}%",
        report.summary.case_parity * 100.0,
        report.summary.weighted_requirement_parity * 100.0
    );

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,iron_pony_parity=debug,xtask=debug"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_ansi(true)
        .init();
}
