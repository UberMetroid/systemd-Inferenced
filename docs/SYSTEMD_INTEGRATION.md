# systemd Integration Guide for systemd-inferenced

## 1. Slices & cgroups v2 Hierarchy

`systemd-inferenced` relies on cgroups v2 to enforce resource bounds:

```text
/sys/fs/cgroup/ai.slice
├── ai-sentry.slice           # Dedicated emergency supervisor enclave
│   ├── MemoryMin=2G          # Guaranteed locked host RAM
│   ├── CPUWeight=1000        # Real-time scheduling priority
│   └── ManagedOOMPreference=avoid
├── ai-interactive.slice      # User chat and tool-calling completions
│   ├── CPUWeight=500
│   └── MemoryHigh=70%
└── ai-batch.slice            # Offline bulk diffusion, embeddings, Whisper
    ├── CPUWeight=50
    └── MemoryHigh=50%
```

## 2. Socket Activation (`sd_listen_fds`)

`systemd-inferenced.socket` manages listener streams on behalf of engines:
* `/run/systemd-inferenced/sentry.sock`: Dedicated triage connection for `systemd-sentry`.
* `127.0.0.1:11434`: Local OpenAI/Ollama-compatible gateway.
* `/run/systemd-inferenced/io.systemd.inferenced1`: Varlink IPC socket.

## 3. Sandboxing & Zero-Trust Privileges

The daemon executes as an unprivileged system user (`inferenced:inferenced`):
* `ProtectSystem=strict`
* `ProtectHome=yes`
* `MemoryDenyWriteExecute=yes`
* `NoNewPrivileges=yes`
* Group membership: `render` and `video` for access to `/dev/dri/renderD*` and `/dev/accel/*`.
