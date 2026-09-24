# Project: systemd-inferenced

## Architecture
systemd-inferenced is a 100% pure Rust unprivileged heterogeneous hardware arbiter and model lifecycle broker.
- **`crates/inferenced-core`**: Core engine providing hardware discovery (DRM GPUs, NPUs, UMA APUs, CPU matrix extensions AMX/AVX-512), resource leasing arbiter, two-tier preemption coordinator (250ms cooperative yield fallback to `cgroup.freeze` / `SIGSTOP`), sealed `memfd_create` zero-copy memory distribution with `SCM_RIGHTS`, and `madvise`/`zswap` dynamic demand paging.
- **`crates/inferenced-daemon`**: Main daemon implementing systemd pure-Rust socket activation (FDs 3..5), abstract Linux socket (`@`) notification protocol, native Varlink IPC `io.systemd.inferenced1` server with full introspection, out-of-band Sentry emergency triage enclave (`/run/systemd-inferenced/sentry.sock`), and HTTP inference gateway.
- **`crates/inferenctl`**: Modular CLI tool supporting 18 subcommands, global flags (`--json`, `--no-pager`, `--quiet`), shell completions, man pages, and composable Unix stream filter (`stdin | inferenctl exec <model> > stdout`).
- **`qa/`**: Comprehensive test infrastructure supporting Tiers 1-4 requirement-driven opaque-box tests and Tier 5 adversarial tests.
- **`systemd/` & `install/`**: Systemd unit files (`systemd-inferenced.service`, `systemd-inferenced.socket`, `ai.slice`, `ai-sentry.slice`), `install.sh`, and `uninstall.sh`.
- **`docs/` & `website/`**: Full documentation (`CLI_REFERENCE.md`, `VARLINK_SPEC.md`, `SENTRY_ENCLAVE.md`, `SYSTEMD_INTEGRATION.md`) and project website.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Heterogeneous Hardware Discovery | Scan DRM GPUs (`/dev/dri/renderD*`), NPUs (`/dev/accel/*`, Hailo), UMA APUs, and CPU matrix (AMX/AVX-512) via sysfs/procfs | M1 | Survey / R1 |
| 2 | Arbiter Lease State Machine | Priority-based lease allocation (`EmergencyTriage=100`, `Interactive=10`, `Batch=0`), clean borrowing | M1 | Survey / R1 |
| 3 | PSI Memory Pressure Throttling | Read Linux `/proc/pressure/memory` and throttle batch workloads under critical memory pressure | M1 | Survey / R1 |
| 4 | Two-Tier Preemption Coordinator | 250ms cooperative yield timeout fallback to kernel `cgroup.freeze` / `SIGSTOP` (`preempt.rs`) | M1 | Survey / R1 |
| 5 | Sealed memfd & SCM_RIGHTS | Pure-Rust sealed `memfd_create` model sharing with SCM_RIGHTS FD passing via rustix 0.38 (`fd_lease.rs`) | M1 | Survey / R3 |
| 6 | Paging & Reclamation | Linux `madvise(Advice::LinuxDontNeed / Advice::WillNeed)` and zswap management (`paging.rs`) | M1 | Survey / R3 |
| 7 | Pure Rust Socket Activation | Adopt systemd pre-bound FDs 3..5 without port conflicts (`LISTEN_FDS`, `activation.rs`) | M2 | Survey / R4 |
| 8 | Abstract Socket Notification | Send `READY=1`, `STATUS=...`, `WATCHDOG=1`, `STOPPING=1` to abstract Linux sockets (`@`) via rustix | M2 | Survey / R4 |
| 9 | Native Varlink IPC & Introspection | Implement `io.systemd.inferenced1` JSON stream over `/run/systemd-inferenced/io.systemd.inferenced1` with `org.varlink.service` | M2 | Survey / R4 |
| 10 | Sentry Emergency Triage Enclave | Out-of-band triage socket `/run/systemd-inferenced/sentry.sock`, `MemoryMin=2G`, NPU/CPU pin, GPU crash survival | M2 | Survey / R2 |
| 11 | CLI Config & Diagnostics | `cat-config`, `check-config`, `dump`, `inspect` commands in `inferenctl` | M3 | User / Survey |
| 12 | CLI Model Operations | `register`, `warm`, `pin`, `evict`, `models` commands in `inferenctl` | M3 | User / Survey |
| 13 | CLI Verification & Benchmarking | `test-triage` (synthetic sentry diagnostic ping), `benchmark` commands in `inferenctl` | M3 | User / Survey |
| 14 | CLI System & Stream Management | `status`, `planes`, `monitor`, `leases`, `freeze`, `thaw` commands in `inferenctl` | M3 | User / Survey |
| 15 | Composable Unix Stream Filter | `stdin | inferenctl exec <model> > stdout` piping tokens to stdout with exit code 0 | M3 | User / R3 |
| 16 | CLI Tooling & Global Flags | `completions` (bash/zsh/fish), `man`, `--json`, `--no-pager`, `--quiet` flags | M3 | User / Survey |
| 17 | Systemd Packaging & Hardening | Units: `systemd-inferenced.service`, `systemd-inferenced.socket`, `ai.slice`, `ai-sentry.slice` | M4 | Survey / R2, R4 |
| 18 | Installer & Uninstaller | `install/install.sh` and `install/uninstall.sh` scripts for non-root / root installation | M4 | Survey / R5 |
| 19 | Technical Documentation | `docs/CLI_REFERENCE.md`, `docs/VARLINK_SPEC.md`, `docs/SENTRY_ENCLAVE.md`, `docs/SYSTEMD_INTEGRATION.md` | M4 | Survey / R5 |
| 20 | Project Website Updates | Modern documentation website updated with CLI examples, stream piping, and triage enclave details | M4 | Survey / R5 |
| 21 | Full Workspace Compilation & Test | `cargo test --workspace` passes 100% across all unit, edge, fuzz, and integration suites | M5 | Acceptance / R5 |
| 22 | Line-Count Constraint Verification | Zero files exceeding 256 lines of code across all `.rs`, `.sh`, `.html`, `.css`, `.md`, `.toml` files | M5 | Acceptance / R5 |
| 23 | Pure Rust Dependency Verification | `ldd` verification of binaries showing zero linkage to `libsystemd.so` or `libdbus-1.so` | M5 | Acceptance / R5 |
| 24 | Live System & IPC Validation | Validate `varlinkctl info`, Unix stream filter piping, and Sentry triage emergency preemption | M5 | Acceptance / R2-R4 |
| 25 | Comprehensive E2E Test Suite | 4-Tier requirement-driven opaque-box test suite + Tier 5 adversarial coverage hardening | E2E | Acceptance / R5 |
| 26 | Cross-Architecture Netlink Portability | Replace inline asm with rustix raw socket + libc bind across x86_64, aarch64, riscv64 | M1_H2 | R1 (Hardening) |
| 27 | Embedded Standalone Inhibitor Fallback | 3-tier fallback (systemd-inhibit -> flock lockfile -> Unix bus) + child zombie reaping | M2_H2 | R2 (Hardening) |
| 28 | Adversarial Stress & Fault Injection | PSI saturation churn, UMA preemption/thaw storms, SCM_RIGHTS fanout, rogue disconnects | M3_H2 | R3 (Hardening) |
| 29 | Zero-Warning Compiler Hygiene & LOC | 0 warnings on all targets, <= 256 LOC per file repository-wide, 100% pure Rust | M4_H2 | R4 (Hardening) |
| 30 | Adversarial Multi-Agent Gate | Reviewers, Challengers, and Forensic Auditor verification gate | M5_H2 | Acceptance |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Core Compute & Memory Engine | `crates/inferenced-core` | none | DONE |
| M2 | Systemd Daemon, Varlink & Sentry | `crates/inferenced-daemon` | M1 | DONE |
| M3 | Expanded CLI Suite & Unix Filter | `crates/inferenctl` | M1, M2 | DONE |
| M4 | Packaging, Installer & Docs | `install/`, `systemd/`, `docs/`, `website/` | M2, M3 | DONE |
| M5 | Final Integration & Live Verification | Pass 100% E2E test suite (Tiers 1-4), Tier 5 adversarial hardening | M1-M4, E2E | DONE |
| E2E | E2E Testing Track | Comprehensive opaque-box test suite (Tiers 1-4) in `qa/`, `TEST_READY.md` | none (parallel) | DONE |
| M1_H2 | Core Netlink Portability (R1) | `crates/inferenced-core/src/netlink.rs`, `Cargo.toml` | none | DONE |
| M2_H2 | Daemon Inhibitor Fallback (R2) | `crates/inferenced-daemon/src/inhibit.rs` | none | DONE |
| M3_H2 | Adversarial Stress & Fault Injection (R3) | `crates/inferenced-core/src/psi.rs`, `qa/edge/**` | M1_H2, M2_H2 | DONE |
| M4_H2 | Compiler Hygiene & LOC Verification (R4) | Repository-wide compiler check, LOC verification, pure Rust check | M3_H2 | DONE |
| M5_H2 | Final Verification Gate | Reviewers (2), Challengers (2), Forensic Auditor (1) | M1_H2-M4_H2 | DONE |

## Interface Contracts
### `inferenced-core` ↔ `inferenced-daemon`
- `HardwareTopology::discover() -> Result<Self, TopologyError>`: Enumerates all available DRM, NPU, UMA, and CPU compute planes.
- `Arbiter::new(topology: HardwareTopology) -> Self`: Initializes priority arbiter.
- `Arbiter::acquire_lease(&mut self, request: LeaseRequest) -> Result<LeaseGrant, LeaseError>`: Acquires compute lease according to priority (`EmergencyTriage=100`, `Interactive=10`, `Batch=0`).
- `PreemptCoordinator::preempt_lease(&self, lease_id: LeaseId) -> Result<(), PreemptError>`: Emits cooperative yield with 250ms timeout; triggers `cgroup.freeze` or `SIGSTOP` on expiration.
- `MemfdPaging::create_sealed_model(name: &str, data: &[u8]) -> Result<OwnedFd, PagingError>`: Creates sealed `memfd` and applies `madvise`.
- `FdLease::send_fd(socket: &UnixDatagram, fd: BorrowedFd) -> Result<(), FdError>`: SCM_RIGHTS passing.
- `netlink::open_uevent_socket() -> Result<OwnedFd>`: Open and bind portable Netlink KOBJECT_UEVENT multicast listener without inline asm.

### `inferenced-daemon` ↔ `inferenctl` / Clients
- **Inhibitor Fallback**: Primary `systemd-inhibit`, secondary file descriptor advisory lock via `rustix::fs::flock` on `/run/systemd-inferenced/inhibit.lock`, tertiary Unix bus socket.
- **Varlink IPC (`io.systemd.inferenced1`)**: Over `/run/systemd-inferenced/io.systemd.inferenced1` (FD 3):
  - `io.systemd.inferenced1.GetStatus() -> (status: Status)`
  - `io.systemd.inferenced1.ListPlanes() -> (planes: []ComputePlane)`
  - `io.systemd.inferenced1.ListLeases() -> (leases: []LeaseInfo)`
  - `io.systemd.inferenced1.AcquireLease(plane_id: string, priority: int) -> (lease_id: string)`
  - `io.systemd.inferenced1.ReleaseLease(lease_id: string) -> ()`
  - `org.varlink.service.GetInfo() -> (vendor: string, product: string, version: string, url: string, interfaces: []string)`
  - `org.varlink.service.GetInterfaceDescription(interface: string) -> (description: string)`
- **Sentry Emergency Socket**: Over `/run/systemd-inferenced/sentry.sock` (FD 4):
  - Emergency protocol: JSON command `{"action":"triage_ping"}` / `{"action":"emergency_preempt"}` responding with status and resource allocation.
- **Gateway / Stream Filter**: Over `/run/systemd-inferenced/io.sock` or `127.0.0.1:11434` (FD 5):
  - Endpoint `/api/generate`: Streams JSON or raw tokens line-by-line for `inferenctl exec`.

## Code Layout
- Every source file MUST strictly contain <= 256 lines of code.
- Zero C-library dependencies (no `libsystemd.so`, no `libdbus-1.so`).
- Dedicated file ownership for Round 2:
  - Worker M1_H2 exclusively owns: `crates/inferenced-core/src/netlink.rs`, `crates/inferenced-core/Cargo.toml`
  - Worker M2_H2 exclusively owns: `crates/inferenced-daemon/src/inhibit.rs`
  - Worker M3_H2 exclusively owns: `crates/inferenced-core/src/psi.rs`, `qa/edge/psi_churn_stress_tests.rs`, `qa/edge/uma_thaw_storm_tests.rs`, `qa/edge/scm_rights_fanout_tests.rs`, `qa/edge/rogue_disconnect_tests.rs`, `qa/edge/mod.rs`
  - Reviewers / Challengers / Auditor: Read-only inspection across all workspace files.
