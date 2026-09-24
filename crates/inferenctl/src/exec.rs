use crate::client::VarlinkClient;
use anyhow::Result;
use serde_json::json;
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;

/// Composable Unix stream filter:
/// Reads input prompt from stdin, dispatches inference over Varlink IPC,
/// and streams output tokens directly to stdout in real-time.
pub fn run_exec(
    socket_path: impl AsRef<Path>,
    model: &str,
    prompt_arg: Option<String>,
) -> Result<()> {
    let mut prompt = String::new();

    if let Some(arg) = prompt_arg {
        prompt = arg;
    } else {
        let mut stdin = io::stdin();
        if !stdin.is_terminal() {
            stdin.read_to_string(&mut prompt)?;
        }
    }

    if prompt.trim().is_empty() {
        prompt = "Hello, systemd-inferenced!".into();
    }

    let mut client = VarlinkClient::connect(socket_path)?;
    let mut stdout = io::stdout();

    client.stream_call(
        "io.systemd.inferenced1.StreamInference",
        Some(json!({
            "model": model,
            "prompt": prompt,
        })),
        |msg| {
            if let Some(params) = msg.get("parameters") {
                if let Some(chunk) = params.get("chunk").and_then(|v| v.as_str()) {
                    let _ = stdout.write_all(chunk.as_bytes());
                    let _ = stdout.flush();
                }
            }
            Ok(())
        },
    )?;

    let _ = stdout.write_all(b"\n");
    let _ = stdout.flush();
    Ok(())
}
