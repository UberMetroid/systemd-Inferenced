# systemd-sentry Emergency Triage Enclave

## Problem Statement

When an AI workload misbehaves (e.g. CUDA memory leak, kernel page table starvation, or driver deadlock), `systemd-sentry` intercepts the crash signal over D-Bus and attempts to diagnose the causal incident window.

If Sentry relies on the primary GPU or standard inference queues:
1. Sentry's inference request queues behind the very job that caused the failure.
2. A kernel GPU driver panic (`amdgpu` ring timeout or CUDA fault) makes all `/dev/dri` access hang indefinitely.
3. Sentry is blinded and unable to triage the node.

## The Out-of-Band Enclave Solution

`systemd-inferenced` resolves this via hardware diversification:

1. **Hardware Reservation**:
   * **NPU First**: If an NPU (`/dev/accel/*` or Hailo) is detected, Sentry's emergency triage model is permanently mapped to the NPU's private SRAM/DRAM.
   * **Pinned CPU Fallback**: If no NPU is present, 2 CPU cores and 2GB of host RAM are pinned with `MemoryMin=2G` in `ai-sentry.slice`.
2. **Dedicated IPC Channel**:
   * Sentry communicates directly via `/run/systemd-inferenced/sentry.sock`.
   * Requests tagged with `LeasePriority::EmergencyTriage` never wait in user queues and preempt lower-priority batch leases immediately.
3. **Driver Fault Immunity**:
   * Because the enclave runs out-of-band (on NPU or CPU matrix extensions), it survives catastrophic discrete GPU driver hangs without interruption.
