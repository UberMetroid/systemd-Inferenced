# E2E Test Infra: systemd-inferenced

## Test Philosophy
- Opaque-box, requirement-driven derived strictly from `ORIGINAL_REQUEST.md` and user-facing contracts.
- Zero dependency on internal implementation design.
- Methodology: Category-Partition + Boundary Value Analysis (BVA) + Pairwise Combinatorial + Real-World Workload Testing.

## Feature Inventory & Test Coverage Goals
| # | Feature | Requirement | Tier 1 | Tier 2 | Tier 3 |
|---|---------|-------------|:------:|:------:|:------:|
| 1 | Heterogeneous Hardware Discovery | R1 | 5 | 5 | ✓ |
| 2 | Two-Tier Preemption & Timeouts | R1 | 5 | 5 | ✓ |
| 3 | Sentry Emergency Triage Enclave | R2 | 5 | 5 | ✓ |
| 4 | Sealed memfd & SCM_RIGHTS Zero-Copy | R3 | 5 | 5 | ✓ |
| 5 | Kernel madvise & zswap Paging | R3 | 5 | 5 | ✓ |
| 6 | Unix Stream Filter CLI (`inferenctl exec`) | R3 | 5 | 5 | ✓ |
| 7 | Pure Rust Systemd Socket Activation (FDs 3..5) | R4 | 5 | 5 | ✓ |
| 8 | Abstract Linux Socket Notification (`@`) | R4 | 5 | 5 | ✓ |
| 9 | Native Varlink IPC (`io.systemd.inferenced1`) | R4 | 5 | 5 | ✓ |
| 10 | Expanded CLI Command Suite (18 commands + flags) | R5 / User | 10 | 5 | ✓ |

## Test Architecture
- **Directory Layout**:
  - `qa/unit/`: Component-level unit tests for topology, parsing, and lease logic.
  - `qa/edge/`: Boundary conditions, memory pressure, timeout limits, and failure injection.
  - `qa/integration/`: End-to-end integration tests for Varlink IPC, socket activation, Sentry triage channel, and Unix stream filter.
  - `qa/fuzz/`: Fuzz testing for malformed Varlink payloads, socket traffic, and corrupted memory models.
- **Test Runner**:
  - Full suite command: `cargo test --workspace`
  - Integration binary: `cargo test -p inferenced-qa --test integration`
  - Pass/fail semantics: All assertions must pass with exit code 0. Zero warnings in production builds.

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Complexity |
|---|----------|--------------------|------------|
| 1 | High-Volume Token Streaming | Unix stream filter (`stdin | inferenctl exec <model> > stdout`), zero-copy memfd, socket activation | High |
| 2 | Emergency Kernel Driver Panic Triage | Simulated GPU hang, Sentry out-of-band enclave request over `sentry.sock`, emergency preemption of batch leases | Critical |
| 3 | Memory Starvation & PSI Paging | Heavy background allocation, Linux PSI critical trigger, automatic model madvise `LinuxDontNeed`, zswap reclamation | High |
| 4 | Varlink Service Introspection & Management | `varlinkctl info`, interface validation, plane discovery, and lease acquisition via Varlink IPC | Medium |
| 5 | Multi-Tenant Lease Arbitration & Cooperative Yield | Simultaneous interactive and batch leases, cooperative yield with 250ms deadline, cgroup freezing fallback | High |

## Coverage Thresholds
- **Tier 1**: >= 5 per feature (50+ tests)
- **Tier 2**: >= 5 per feature (50+ tests)
- **Tier 3**: Pairwise coverage of major feature combinations (10+ interaction tests)
- **Tier 4**: 5 realistic end-to-end workload scenarios
- **Total Minimum**: >= 115 tests passing across `cargo test --workspace`.
