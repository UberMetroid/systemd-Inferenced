use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "inferenctl")]
#[command(about = "Control and inspect systemd-inferenced hardware arbitration and model residency")]
#[command(version)]
pub struct Cli {
    #[arg(long, default_value = "/run/systemd-inferenced/io.systemd.inferenced1", global = true)]
    pub socket: PathBuf,

    #[arg(long, default_value = "/run/systemd-inferenced/sentry.sock", global = true)]
    pub sentry_socket: PathBuf,

    #[arg(short, long, default_value = "/etc/systemd/inferenced.conf", global = true)]
    pub config: PathBuf,

    #[arg(long, global = true)]
    pub json: bool,

    #[arg(long, global = true)]
    pub no_pager: bool,

    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    #[command(about = "Display daemon operational health, socket endpoints, cgroup slice, and compute planes")]
    Status,
    #[command(about = "List discovered compute planes (dGPU, NPU, UMA, CPU) and memory pools")]
    Planes,
    #[command(about = "Display active resource leases, priorities, and client PIDs")]
    Leases,
    #[command(about = "Stream real-time Linux PSI pressure and active lease updates")]
    Monitor {
        #[arg(short = 'n', long, default_value = "5")]
        count: usize,
    },
    #[command(about = "Composable Unix stream filter: read prompt from stdin and stream tokens to stdout")]
    Exec {
        model: String,
        prompt: Option<String>,
    },
    #[command(about = "Freeze an active compute lease via cgroup.freeze or SIGSTOP")]
    Freeze {
        lease_id: String,
    },
    #[command(about = "Resume an active compute lease that was previously frozen")]
    Thaw {
        lease_id: String,
    },
    #[command(about = "List registered models, formats, memory footprints, and residency states")]
    Models,
    #[command(about = "Register a local model file for arbiter residency tracking")]
    Register {
        id: String,
        #[arg(short, long, default_value = "GGUF")]
        format: String,
        #[arg(short, long)]
        path: Option<String>,
        #[arg(short, long, default_value = "4294967296")]
        bytes: u64,
    },
    #[command(about = "Pre-fault and warm model pages into device memory")]
    Warm {
        id: String,
    },
    #[command(about = "Pin a model to a compute plane for emergency triage")]
    Pin {
        id: String,
        #[arg(short, long)]
        plane: Option<String>,
        plane_arg: Option<String>,
    },
    #[command(about = "Evict a resident model from accelerator memory back to disk or zswap")]
    Evict {
        id: String,
    },
    #[command(about = "Print parsed global configuration")]
    CatConfig,
    #[command(about = "Validate configuration syntax, paths, and PSI thresholds")]
    CheckConfig,
    #[command(about = "Dump complete internal state (arbiter table, topology, memory pools) as JSON")]
    Dump,
    #[command(about = "Inspect low-level hardware attributes of a compute plane")]
    Inspect {
        plane_id: String,
    },
    #[command(about = "Send synthetic systemd-sentry emergency triage diagnostic ping")]
    TestTriage,
    #[command(about = "Execute throughput and latency benchmarks over IPC")]
    Benchmark {
        #[arg(short, long, default_value = "100")]
        iterations: usize,
    },
    #[command(about = "Generate tab-completion scripts for shell environments")]
    Completions {
        #[arg(default_value = "bash")]
        shell: String,
    },
    #[command(about = "Output the man page for inferenctl(1) in troff format")]
    Man,
}
