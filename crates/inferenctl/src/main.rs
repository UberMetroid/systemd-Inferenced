mod client;
mod cmd_bench;
mod cmd_config;
mod cmd_model;
mod cmd_system;
mod cmd_tooling;
mod exec;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "inferenctl")]
#[command(about = "Control and inspect systemd-inferenced hardware arbitration and model residency")]
#[command(version)]
struct Cli {
    #[arg(long, default_value = "/run/systemd-inferenced/io.systemd.inferenced1", global = true)]
    socket: PathBuf,

    #[arg(long, default_value = "/run/systemd-inferenced/sentry.sock", global = true)]
    sentry_socket: PathBuf,

    #[arg(short, long, default_value = "/etc/systemd/inferenced.conf", global = true)]
    config: PathBuf,

    #[arg(long, global = true)]
    json: bool,

    #[arg(long, global = true)]
    no_pager: bool,

    #[arg(short, long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Status,
    Planes,
    Leases,
    Monitor {
        #[arg(short, long, default_value = "5")]
        count: usize,
    },
    Exec {
        model: String,
        prompt: Option<String>,
    },
    Freeze {
        lease_id: String,
    },
    Thaw {
        lease_id: String,
    },
    Models,
    Register {
        id: String,
        #[arg(short, long, default_value = "GGUF")]
        format: String,
        #[arg(short, long)]
        path: String,
        #[arg(short, long, default_value = "4294967296")]
        bytes: u64,
    },
    Warm {
        id: String,
    },
    Pin {
        id: String,
        plane: String,
    },
    Evict {
        id: String,
    },
    CatConfig,
    CheckConfig,
    Dump,
    Inspect {
        plane_id: String,
    },
    TestTriage,
    Benchmark {
        #[arg(short, long, default_value = "100")]
        iterations: usize,
    },
    Completions {
        #[arg(default_value = "bash")]
        shell: String,
    },
    Man,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Status => cmd_system::run_status(&cli.socket, cli.json)?,
        Commands::Planes => cmd_system::run_planes(&cli.socket, cli.json)?,
        Commands::Leases => cmd_system::run_leases(&cli.socket, cli.json)?,
        Commands::Monitor { count } => cmd_system::run_monitor(&cli.socket, count)?,
        Commands::Exec { model, prompt } => exec::run_exec(&cli.socket, &model, prompt)?,
        Commands::Freeze { lease_id } => cmd_system::run_freeze(&cli.socket, &lease_id)?,
        Commands::Thaw { lease_id } => cmd_system::run_thaw(&cli.socket, &lease_id)?,
        Commands::Models => cmd_model::run_models(&cli.socket, cli.json)?,
        Commands::Register { id, format, path, bytes } => {
            cmd_model::run_register(&cli.socket, &id, &format, &path, bytes)?
        }
        Commands::Warm { id } => cmd_model::run_warm(&cli.socket, &id)?,
        Commands::Pin { id, plane } => cmd_model::run_pin(&cli.socket, &id, &plane)?,
        Commands::Evict { id } => cmd_model::run_evict(&cli.socket, &id)?,
        Commands::CatConfig => cmd_config::run_cat_config(&cli.config)?,
        Commands::CheckConfig => cmd_config::run_check_config(&cli.config)?,
        Commands::Dump => cmd_config::run_dump(&cli.socket)?,
        Commands::Inspect { plane_id } => cmd_config::run_inspect(&cli.socket, &plane_id)?,
        Commands::TestTriage => cmd_bench::run_test_triage(&cli.sentry_socket)?,
        Commands::Benchmark { iterations } => cmd_bench::run_benchmark(&cli.socket, iterations)?,
        Commands::Completions { shell } => cmd_tooling::run_completions(&shell)?,
        Commands::Man => cmd_tooling::run_man()?,
    }

    Ok(())
}
