# Varlink IPC Specification: `io.systemd.inferenced1`

`systemd-inferenced` implements native Varlink IPC over a Unix domain socket:
`/run/systemd-inferenced/io.systemd.inferenced1`

Varlink protocol messages are UTF-8 JSON objects terminated by a single null byte (`\0`).

---

## 1. Interface Definition (IDL)

```varlink
interface io.systemd.inferenced1

type ComputePlane (
  id: string,
  name: string,
  kind: string,
  total_memory: int,
  available_memory: int,
  is_triage_reserved: bool
)

type LeaseInfo (
  id: string,
  plane_id: string,
  allocated_memory: int,
  priority: string,
  state: string,
  client_unit: ?string,
  client_pid: ?int
)

type ModelInfo (
  id: string,
  format: string,
  path: string,
  estimated_memory: int,
  placement: string
)

method GetTopology() -> (
  planes: []ComputePlane,
  total_ram: int,
  available_ram: int,
  cpu_cores: int
)

method GetPressure() -> (
  level: string,
  cpu_some: float,
  memory_some: float,
  io_some: float
)

method AcquireLease(
  priority: string,
  memory_bytes: int,
  plane: ?string,
  unit: ?string,
  pid: ?int
) -> (
  lease_id: string,
  plane_id: string,
  allocated_memory: int
)

method ReleaseLease(lease_id: string) -> ()
method Yield(lease_id: string) -> ()
method Freeze(lease_id: string) -> ()
method Thaw(lease_id: string) -> ()
method ListLeases() -> (leases: []LeaseInfo)
method ListModels() -> (models: []ModelInfo)

method RegisterModel(
  id: string,
  format: string,
  path: string,
  estimated_bytes: int
) -> ()

method StreamInference(
  model: string,
  prompt: string
) -> (
  chunk: string
)

error MethodNotFound (method: string)
error InvalidParameter (parameter: string)
error ResourceExhaustion (plane: string, requested: int, available: int)
error PlaneNotFound (plane: string)
error LeaseNotFound (lease_id: string)
error ModelNotFound (model: string)
```

---

## 2. Standard Service Introspection (`org.varlink.service`)

The daemon implements the standard Varlink introspection interface:

```varlink
interface org.varlink.service

method GetInfo() -> (
  vendor: string,
  product: string,
  version: string,
  url: string,
  interfaces: []string
)

method GetInterfaceDescription(interface: string) -> (description: string)

error InterfaceNotFound (interface: string)
error MethodNotFound (method: string)
error MethodNotImplemented (method: string)
error InvalidParameter (parameter: string)
```

### Introspection Example
```bash
$ varlinkctl info unix:/run/systemd-inferenced/io.systemd.inferenced1
Vendor: systemd-inferenced
Product: systemd-inferenced
Version: 0.1.0
URL: https://github.com/UberMetroid/systemd-Inferenced
Interfaces:
  org.varlink.service
  io.systemd.inferenced1
```

---

## 3. Method Specifications

### `GetTopology`
Returns discovered hardware compute planes (DRM GPU, NPU, UMA APU, CPU matrix extensions AMX/AVX-512) and system memory metrics.

### `GetPressure`
Returns Linux Pressure Stall Information (PSI) for CPU, memory, and I/O.
- `level`: `"Normal"`, `"Moderate"`, or `"Critical"`.

### `AcquireLease`
Acquires a resource slice on an accelerator plane.
- `priority`: `"EmergencyTriage"` (weight 100), `"Interactive"` (weight 10), or `"Batch"` (weight 0).
- `memory_bytes`: Requested allocation in bytes.
- `plane`: Target plane ID, or `None` for automatic arbitration.
- `unit`: Calling systemd cgroup unit name (e.g., `user@1000.service`).
- `pid`: Calling process ID for tracking.

### `ReleaseLease`
Releases an active lease, triggering memory reclamation or unfreezing queued batch workloads.

### `Yield`, `Freeze`, `Thaw`
Cooperative and kernel-enforced preemption controls:
- `Yield`: Emits a cooperative yield signal with a 250ms deadline.
- `Freeze`: Enforces preemption via `cgroup.freeze` or `SIGSTOP`.
- `Thaw`: Resumes a frozen workload.

### `StreamInference`
Streams generation output chunk-by-chunk using Varlink `continues: true` flags until the final token is sent (`continues: false`).

---

## 4. Error Mapping

| Varlink Error | HTTP / Exit Mapping | Meaning |
|---------------|---------------------|---------|
| `ResourceExhaustion` | 503 / 1 | Insufficient VRAM/RAM on plane |
| `PlaneNotFound` | 404 / 1 | Specified compute plane ID invalid |
| `LeaseNotFound` | 404 / 1 | Specified lease UUID does not exist |
| `ModelNotFound` | 404 / 1 | Model ID has not been registered |
| `InvalidParameter` | 400 / 1 | Malformed request parameter |

---

## 5. Command-Line Testing with `varlinkctl`

```bash
# Query interface description
varlinkctl introspect unix:/run/systemd-inferenced/io.systemd.inferenced1 io.systemd.inferenced1

# Call GetTopology
varlinkctl call unix:/run/systemd-inferenced/io.systemd.inferenced1 io.systemd.inferenced1.GetTopology '{}'

# Acquire an Interactive lease for 1GB
varlinkctl call unix:/run/systemd-inferenced/io.systemd.inferenced1 io.systemd.inferenced1.AcquireLease \
  '{"priority": "Interactive", "memory_bytes": 1073741824}'
```
