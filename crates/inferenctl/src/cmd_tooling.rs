use anyhow::Result;

pub fn run_completions(shell: &str) -> Result<()> {
    match shell.to_lowercase().as_str() {
        "bash" => {
            println!("{}", r#"# bash completion for inferenctl
_inferenctl() {
    local cur prev words cword
    _init_completion || return
    local commands="status planes leases monitor exec freeze thaw register warm pin evict models cat-config check-config dump inspect test-triage benchmark completions man"
    if [[ $cword -eq 1 ]]; then
        COMPREPLY=( $(compgen -W "$commands" -- "$cur") )
        return 0
    fi
}
complete -F _inferenctl inferenctl
"#);
        }
        "zsh" => {
            println!("{}", r#"#compdef inferenctl
_inferenctl() {
    local -a commands
    commands=(
        'status:Show AI compute status and active leases'
        'planes:List discovered compute planes'
        'leases:List active compute slice leases'
        'monitor:Live monitor of hardware pressure and bus saturation'
        'exec:Unix stream filter for model inference'
        'freeze:Freeze a compute lease'
        'thaw:Thaw a compute lease'
        'models:List registered models'
        'register:Register a model'
        'warm:Warm model weights in memory'
        'pin:Pin model for emergency triage'
        'evict:Evict model from active memory'
        'cat-config:Print configuration'
        'check-config:Validate configuration syntax'
        'dump:Dump internal state as JSON'
        'inspect:Inspect compute plane details'
        'test-triage:Synthetic Sentry diagnostic ping'
        'benchmark:Benchmark IPC and lease throughput'
        'completions:Generate shell completions'
        'man:Generate man page'
    )
    _describe 'command' commands
}
_inferenctl "$@"
"#);
        }
        _ => {
            eprintln!("Unsupported shell: {}. Supported shells: bash, zsh", shell);
        }
    }
    Ok(())
}

pub fn run_man() -> Result<()> {
    println!("{}", r#".TH INFERENCTL 1 "September 2026" "systemd-inferenced 0.1.0" "User Commands"
.SH NAME
inferenctl \- Control and inspect systemd-inferenced hardware arbitration
.SH SYNOPSIS
.B inferenctl
[\fIOPTIONS\fR] \fICOMMAND\fR [\fIARGS\fR]
.SH DESCRIPTION
\fBinferenctl\fR communicates with \fBsystemd-inferenced\fR over Varlink IPC
to manage compute slice leases, monitor memory bus saturation, and stream
zero-copy inference filters.
.SH COMMANDS
.TP
\fBstatus\fR
Show system-wide AI compute status, PSI pressure, and active leases.
.TP
\fBplanes\fR
List all discovered compute planes (dGPU, UMA, NPU, CPU-Matrix).
.TP
\fBexec\fR \fIMODEL\fR [\fIPROMPT\fR]
Composable Unix stream filter: reads prompt from stdin and writes tokens to stdout.
.TP
\fBtest-triage\fR
Send a synthetic systemd-sentry emergency triage ping.
.SH AUTHORS
systemd-inferenced contributors.
"#);
    Ok(())
}
