mod cli;
mod client;
mod cmd;
mod format;

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Status => cmd::system::run_status(&cli.socket, cli.json)?,
        Commands::Planes => cmd::system::run_planes(&cli.socket, cli.json)?,
        Commands::Leases => cmd::system::run_leases(&cli.socket, cli.json)?,
        Commands::Monitor { count } => cmd::system::run_monitor(&cli.socket, count)?,
        Commands::Exec { model, prompt } => {
            cmd::stream::run_exec(&cli.socket, &model, prompt, cli.quiet)?
        }
        Commands::Freeze { lease_id } => cmd::control::run_freeze(&cli.socket, &lease_id)?,
        Commands::Thaw { lease_id } => cmd::control::run_thaw(&cli.socket, &lease_id)?,
        Commands::Models => cmd::models::run_models(&cli.socket, cli.json)?,
        Commands::Register { id, format, path, bytes } => {
            cmd::models::run_register(&cli.socket, &id, &format, path.as_deref(), bytes)?
        }
        Commands::Warm { id } => cmd::models::run_warm(&cli.socket, &id)?,
        Commands::Pin { id, plane, plane_arg } => {
            let target = plane.as_deref().or(plane_arg.as_deref());
            cmd::models::run_pin(&cli.socket, &id, target)?
        }
        Commands::Evict { id } => cmd::models::run_evict(&cli.socket, &id)?,
        Commands::CatConfig => cmd::config::run_cat_config(&cli.config)?,
        Commands::CheckConfig => cmd::config::run_check_config(&cli.config)?,
        Commands::Dump => cmd::config::run_dump(&cli.socket)?,
        Commands::Inspect { plane_id } => cmd::config::run_inspect(&cli.socket, &plane_id, cli.json)?,
        Commands::TestTriage => cmd::triage::run_test_triage(&cli.sentry_socket, cli.quiet)?,
        Commands::Benchmark { iterations } => cmd::tooling::run_benchmark(&cli.socket, iterations)?,
        Commands::Completions { shell } => cmd::tooling::run_completions(&shell)?,
        Commands::Man => cmd::tooling::run_man()?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_structure_and_debug_assertions() {
        Cli::command().debug_assert();
    }

    #[test]
    fn test_cli_parse_exec_command() {
        let args = vec!["inferenctl", "exec", "llama3", "Hello world"];
        let parsed = Cli::try_parse_from(args).expect("Failed to parse exec");
        match parsed.command {
            Commands::Exec { model, prompt } => {
                assert_eq!(model, "llama3");
                assert_eq!(prompt, Some("Hello world".into()));
            }
            _ => panic!("Expected Exec variant"),
        }
    }

    #[test]
    fn test_cli_parse_global_flags() {
        let args = vec!["inferenctl", "--json", "--quiet", "--no-pager", "status"];
        let parsed = Cli::try_parse_from(args).expect("Failed to parse flags");
        assert!(parsed.json);
        assert!(parsed.quiet);
        assert!(parsed.no_pager);
    }

    #[test]
    fn test_cli_parse_all_command_variants() {
        assert!(Cli::try_parse_from(["inferenctl", "planes"]).is_ok());
        assert!(Cli::try_parse_from(["inferenctl", "leases"]).is_ok());
        assert!(Cli::try_parse_from(["inferenctl", "monitor", "-n", "3"]).is_ok());
        assert!(Cli::try_parse_from(["inferenctl", "models"]).is_ok());
        assert!(Cli::try_parse_from(["inferenctl", "cat-config"]).is_ok());
        assert!(Cli::try_parse_from(["inferenctl", "check-config"]).is_ok());
        assert!(Cli::try_parse_from(["inferenctl", "dump"]).is_ok());
        assert!(Cli::try_parse_from(["inferenctl", "test-triage"]).is_ok());
        assert!(Cli::try_parse_from(["inferenctl", "benchmark"]).is_ok());
        assert!(Cli::try_parse_from(["inferenctl", "completions", "bash"]).is_ok());
        assert!(Cli::try_parse_from(["inferenctl", "man"]).is_ok());
    }

    #[test]
    fn test_offline_stream_execution() {
        let res = cmd::stream::run_exec(
            "/tmp/nonexistent_socket.sock",
            "test-model",
            Some("unit test prompt".into()),
            true,
        );
        assert!(res.is_ok());
    }
}
