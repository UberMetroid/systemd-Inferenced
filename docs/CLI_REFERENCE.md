# inferenctl CLI Reference

`inferenctl` is the command-line control and inspection tool for `systemd-inferenced`.

## Global Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--socket <PATH>` | Varlink IPC socket path | `/run/systemd-inferenced/io.systemd.inferenced1` |
| `--sentry-socket <PATH>` | Out-of-band triage socket | `/run/systemd-inferenced/sentry.sock` |
| `-c, --config <PATH>` | Configuration file path | `/etc/systemd/inferenced.conf` |
| `--json` | Emit structured JSON output | `false` |
| `--no-pager` | Disable interactive pager | `false` |
| `-q, --quiet` | Suppress non-essential output | `false` |

---

## 1. System & Stream Management

### `status`
Displays daemon operational health, socket endpoints, cgroup slice, and compute plane summary.
```bash
inferenctl status [--json]
```

### `planes`
Lists all discovered compute planes (DRM GPUs, NPUs, UMA APUs, CPU AMX/AVX-512) and memory pools.
```bash
inferenctl planes [--json]
```

### `leases`
Displays active resource leases, priorities (`EmergencyTriage`, `Interactive`, `Batch`), and client PIDs.
```bash
inferenctl leases [--json]
```

### `monitor`
Streams real-time Linux Pressure Stall Information (PSI) and active lease updates.
```bash
inferenctl monitor [--count <NUM>]
```

### `exec` (Composable Unix Stream Filter)
Streams input tokens from `stdin` or argument through the specified model and emits tokens to `stdout`.
```bash
inferenctl exec <model> [prompt]
```
**Composable Unix Stream Filter Examples:**
```bash
# Standard pipeline filter
echo "Explain Linux memory cgroups" | inferenctl exec llama-3.2-3b > explanation.md

# Log analysis pipeline
journalctl -u nginx.service -n 50 --no-pager | inferenctl exec sentry-triage-v1

# Structured multi-tool pipeline
cat metrics.json | inferenctl exec summarizer | jq .
```

### `freeze`
Immediately freezes an active compute lease via `cgroup.freeze` or kernel `SIGSTOP`.
```bash
inferenctl freeze <lease_id>
```

### `thaw`
Resumes an active compute lease that was previously frozen.
```bash
inferenctl thaw <lease_id>
```

---

## 2. Model Lifecycle Operations

### `models`
Lists registered models, formats (GGUF, Safetensors), memory footprints, and residency states (`Resident`, `Dormant`, `PagedOut`).
```bash
inferenctl models [--json]
```

### `register`
Registers a local model file with memory footprint estimates for arbiter residency tracking.
```bash
inferenctl register <id> --path <PATH> [--format GGUF] [--bytes <BYTES>]
# Example:
inferenctl register llama3 --path /var/lib/models/llama-3.gguf --bytes 4294967296
```

### `warm`
Pre-faults and warms model pages into device memory via `madvise(Advice::LinuxWillNeed)`.
```bash
inferenctl warm <model_id>
```

### `pin`
Pins a model to a designated compute plane (e.g. NPU SRAM or CPU pinned memory), exempting it from LRU eviction.
```bash
inferenctl pin <model_id> <plane_id>
```

### `evict`
Evicts a resident model from accelerator VRAM/SRAM back to disk or zswap.
```bash
inferenctl evict <model_id>
```

---

## 3. Configuration & Diagnostics

### `cat-config`
Prints the parsed global configuration from `/etc/systemd/inferenced.conf`.
```bash
inferenctl cat-config
```

### `check-config`
Validates configuration syntax, paths, and PSI thresholds.
```bash
inferenctl check-config
```

### `dump`
Dumps complete internal state (arbiter table, topology, memory pools) as JSON for diagnostic capture.
```bash
inferenctl dump
```

### `inspect`
Inspects low-level hardware attributes of a compute plane (PCI ID, NUMA node, DRM render node).
```bash
inferenctl inspect <plane_id>
```

---

## 4. Verification & Benchmarking

### `test-triage`
Sends a synthetic emergency triage diagnostic payload to the out-of-band `systemd-sentry` socket (`/run/systemd-inferenced/sentry.sock`), verifying enclave responsiveness and zero-queue preemption.
```bash
inferenctl test-triage
```

### `benchmark`
Executes throughput and round-trip IPC latency benchmarks over Varlink IPC.
```bash
inferenctl benchmark [--iterations 100]
```

---

## 5. Tooling

### `completions`
Generates tab-completion scripts for shell environments.
```bash
inferenctl completions [bash|zsh|fish] > ~/.bash_completion.d/inferenctl
```

### `man`
Outputs the man page for `inferenctl(1)` in troff format.
```bash
inferenctl man | man -l -
```
