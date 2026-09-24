# systemd-inferenced: Architectural Specification

> **Unprivileged Heterogeneous Hardware Arbiter & Model Lifecycle Broker in Pure Rust**  
> Operating as the Linux system-level compute fabric between `systemd` and competing AI inference workloads.

---

## 1. The Core Philosophy ("The Systemd Way")

In the Linux operating system, memory and compute have historically been managed across CPU timeslices, host RAM pages, and block I/O. The emergence of machine learning runtimes (PyTorch, llama.cpp, vLLM, TensorRT) introduced an unmanaged "wild west":
* Runtimes allocate greedy memory pools, oblivious to peer workloads.
* Unified Memory Architectures (UMA) suffer **memory bus starvation** that freezes the desktop compositor and system services even when host RAM capacity is plentiful.
* Discrete GPU driver hangs (e.g. `amdgpu` ring timeouts, CUDA MMU faults) take down dependent supervisory tools.
* Heavy pipelines trigger unrecoverable CUDA Out-Of-Memory (OOM) kernel panics.

### How Systemd Solves System Problems
When the systemd project tackled DNS (`systemd-resolved`), network links (`systemd-networkd`), user sessions (`systemd-logind`), or containers (`systemd-machined`), it followed four foundational tenets:

1. **Supervision Over Monolithic Duplication**:
   Systemd does not re-implement container runtimes from scratch; it configures namespaces and cgroups v2 boundaries. Similarly, `systemd-inferenced` does not rewrite every diffusion or transformer engine. It supervises their lifecycles, arbitrates hardware compute leases, and freezes/thaws them dynamically.
2. **Resilient Emergency Fallbacks**:
   Every critical systemd daemon has a self-contained fallback (e.g., emergency rescue target, local stub resolver). For `systemd-inferenced`, this means **embedding a dedicated, out-of-band triage core for `systemd-sentry`**. When the machine crashes and discrete GPUs are hung, Sentry's diagnostic channel runs without external dependencies.
3. **Cgroups v2 & Pressure Stall Information (PSI)**:
   Inference workloads are organized into hierarchical slices:
   * `ai.slice`: Root slice for all AI compute.
   * `ai-sentry.slice`: Dedicated emergency supervisor slice (`ManagedOOMPreference=avoid`, `memory.min` reservation).
   * `ai-interactive.slice`: User-facing interactive completions and tool-calling.
   * `ai-batch.slice`: Background embeddings, bulk diffusion generation, audio transcription.
4. **Standard IPC & Companion CLI**:
   * **Varlink (`io.systemd.inferenced1`)** and **D-Bus (`org.freedesktop.inferenced1`)**.
   * **`inferenctl`**: The system administrator CLI for inspecting hardware planes, memory pressure, active leases, and model residency.

---

## 2. Heterogeneous Compute Planes

`systemd-inferenced` abstracts the entire hardware compute plane:

| Plane Kind | Linux Device Path | Characteristics | Arbitration Strategy |
| :--- | :--- | :--- | :--- |
| **Discrete GPU (dGPU)** | `/dev/dri/renderD*`, NVML | High VRAM bandwidth, isolated memory | Compute leases, cooperative offload, cgroup freezing |
| **Integrated UMA** | `/dev/dri/renderD*` (Intel/AMD/ARM APU) | Shared LPDDR/DDR host bus | Linux `resctrl` Memory Bandwidth Allocation (MBA), PSI throttling |
| **NPU / Accelerator** | `/dev/accel/accel*`, `/dev/hailo0` | Dedicated SRAM, ultra-low power, zero GPU dependency | Primary target for `systemd-sentry` emergency enclave |
| **CPU Matrix Plane** | Host CPU (AMX, AVX-512, SME) | NUMA-aware, pinned CPU affinity | Fallback enclave for Sentry; CPU quota enforcement |

---

## 3. The `systemd-sentry` Emergency Triage Enclave

`systemd-sentry` is the zero-trust system supervisor that diagnoses service crashes in real time. If a crash occurs because an inference daemon saturated the GPU or caused kernel page faults, Sentry cannot queue behind that same hung daemon.

`systemd-inferenced` guarantees Sentry's availability via:
1. **Hardware Diversification**:
   * Sentry's triage model runs **out-of-band** on an NPU (`/dev/accel/*`) or dedicated CPU AMX/AVX cores.
   * It never touches discrete GPU drivers, surviving complete GPU kernel deadlocks.
2. **Dedicated Out-of-Band IPC**:
   * Serviced via `/run/systemd-inferenced/sentry.sock`.
   * Requests are tagged with `LeasePriority::EmergencyTriage`, bypassing all batch queues and preempting lower-priority leases instantly.
3. **Kernel Memory Pinning**:
   * Protected with `memory.min` in cgroups v2, preventing host memory eviction under severe system pressure.

---

## 4. Model Demand Paging Lifecycle

Models move through four deterministic states:

```
[Dormant (NVMe Storage)]
         │  (io_uring / mmap zero-copy load)
         ▼
  [Staged (Host RAM)]
         │  (PCIe DMA / NPU SRAM mapping)
         ▼
[Resident (Active Plane)]  ◄───►  [PinnedTriage (systemd-sentry Enclave)]
         │
         │  (Cooperative RPC / cgroup.freeze)
         ▼
[Evicted / Parked]
```
