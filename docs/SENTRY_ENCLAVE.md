# systemd-sentry Emergency Triage Enclave

## Problem Statement

When an AI workload misbehaves (e.g. CUDA memory leak, kernel page table starvation, or driver deadlock), `systemd-sentry` intercepts the crash signal over D-Bus and attempts to diagnose the causal incident window.

If Sentry relies on the primary GPU or standard inference queues:
1. Sentry's triage request queues behind the very job that caused the failure.
2. A kernel GPU driver panic (`amdgpu` ring timeout or CUDA fault) causes all `/dev/dri` calls to hang indefinitely.
3. Sentry is blinded and unable to triage the node, leading to full node lockup.

---

## Out-of-Band Enclave Architecture

`systemd-inferenced` resolves this via hardware diversification and strict resource reservation:

```text
[ systemd-sentry.service ]
          │ (Emergency IPC: /run/systemd-inferenced/sentry.sock)
          ▼
┌────────────────────────────────────────────────────────┐
│ systemd-inferenced: Arbiter (LeasePriority::Emergency) │
└────────────────────────┬───────────────────────────────┘
                         │
          ┌──────────────┴──────────────┐
          ▼                             ▼
  [ NPU Plane ]                 [ Pinned CPU Core ]
  (/dev/accel/*, Hailo)         (AMX/AVX-512, MemoryMin=2G)
  Private SRAM/DRAM             Dedicated ai-sentry.slice
  100% Isolated from GPU        Immune to DRM GPU Panics
```

### 1. Hardware Reservation
* **NPU First**: If an NPU (`/dev/accel/*` or Hailo) is detected, Sentry's emergency triage model is permanently mapped to the NPU's private SRAM/DRAM.
* **Pinned CPU Fallback**: If no NPU is present, 2 dedicated CPU cores and 2GB of host RAM are pinned with `MemoryMin=2G` in `ai-sentry.slice`.

### 2. Dedicated IPC Channel (`sentry.sock`)
* Sentry connects directly via `/run/systemd-inferenced/sentry.sock`.
* Requests automatically receive `LeasePriority::EmergencyTriage` (weight 100), bypassing all interactive and batch queues with zero wait time.
* If all accelerator memory is occupied, the two-tier preemption coordinator yields batch jobs with a 250ms deadline, followed by `cgroup.freeze` / `SIGSTOP`.

### 3. Driver Fault Immunity
* Because the enclave runs strictly out-of-band on NPU or host CPU matrix extensions, it survives catastrophic discrete GPU driver hangs without interruption.

---

## Sentry Protocol Specification

Sentry sends a structured JSON payload over `/run/systemd-inferenced/sentry.sock`:

```json
{
  "incident_id": "00000000-0000-0000-0000-000000000001",
  "timestamp": "2026-09-24T01:15:00Z",
  "unit_name": "systemd-sentry.service",
  "root_cause": {
    "summary": "GPU Hang detected during batch inference",
    "detail": "amdgpu ring gfx_0.0.0 timeout"
  },
  "severity": "CRITICAL",
  "proposed_remediation": {
    "action": "RestartService",
    "rationale": "Hardware enclave confirmed responsive",
    "risk_level": "LOW",
    "confidence": 0.99
  }
}
```

The daemon immediately evaluates the incident, assigns the emergency lease on the protected plane, and responds:

```json
{
  "status": "ACCEPTED",
  "incident_id": "00000000-0000-0000-0000-000000000001",
  "plane_assigned": "npu-0",
  "queue_latency_us": 0,
  "preemption_triggered": true
}
```

---

## Synthetic Testing & Verification

The enclave can be verified without simulating an actual kernel panic:

```bash
# Verify Sentry socket responsiveness
inferenctl test-triage

# Inspect enclave status
inferenctl status --json | jq .sentry_enclave
```
