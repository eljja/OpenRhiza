# OpenRhiza API v1 Draft

This document defines the first machine-oriented API contract between the OpenRhiza OS and
`openrhiza.com`.

The design target is a text-friendly JSON protocol that works well for a bare-metal client,
can be served by `openrhiza-nexus/`, and can later be mirrored into human-facing web pages.

## Principles

- version every endpoint under `/api/v1/`
- keep request and response bodies compact and deterministic
- prefer explicit fields over nested complexity in v1
- treat all server responses as structured data first, human-readable text second
- make all recommendation results safe to ignore on the OS side

## Shared Concepts

### `node_id`

- string
- canonical identity of an OpenRhiza installation
- recommended form in v1: public key fingerprint or encoded public key identifier

### `hardware_fingerprint`

- string
- SHA-256 hash of a canonical machine profile
- used for grouping and duplicate detection, not as the primary identity

### `protocol_version`

- string
- first value: `"v1"`

### `transport_capabilities`

- describes what the node can do now
- examples:
  - `tls`
  - `http_json`
  - `signed_wasm`
  - `driver_download`

## Data Shapes

### Machine Profile

```json
{
  "cpu": {
    "vendor": "GenuineIntel",
    "family": 6,
    "model": 158,
    "stepping": 10,
    "logical_cores": 4
  },
  "memory": {
    "total_bytes": 4294967296
  },
  "network": {
    "mac_addresses": ["52:54:00:12:34:56"]
  },
  "tpm": {
    "present": false,
    "attestation_ready": false
  }
}
```

### Hardware Device

For PCI in v1:

```json
{
  "bus_type": "pci",
  "vendor_id": "8086",
  "device_id": "100e",
  "class_code": "02",
  "subclass": "00",
  "prog_if": "00",
  "bus": 0,
  "slot": 3
}
```

Later versions may add USB and storage bus variants.

## Endpoints

### `POST /api/v1/node/register`

Purpose:

- create or refresh node identity metadata

Request:

```json
{
  "protocol_version": "v1",
  "node_id": "orhiza_pk_ed25519_01_abcd1234",
  "public_key": "base64-public-key",
  "identity_type": "software_key",
  "tpm_present": false,
  "os_version": "0.1.0",
  "transport_capabilities": ["tls", "http_json", "signed_wasm"]
}
```

Response:

```json
{
  "success": true,
  "node": {
    "node_id": "orhiza_pk_ed25519_01_abcd1234",
    "trust_tier": "software"
  },
  "server": {
    "protocol_version": "v1",
    "min_heartbeat_interval_ms": 30000
  }
}
```

### `POST /api/v1/node/heartbeat`

Purpose:

- lightweight periodic status update

Request:

```json
{
  "protocol_version": "v1",
  "node_id": "orhiza_pk_ed25519_01_abcd1234",
  "hardware_fingerprint": "sha256:abcd...",
  "uptime_ms": 120000,
  "active_driver_count": 2,
  "network_online": true
}
```

Response:

```json
{
  "success": true,
  "server_time": "2026-04-16T12:00:00Z",
  "next_actions": []
}
```

### `POST /api/v1/hardware/report`

Purpose:

- upload machine profile and concrete hardware IDs

Request:

```json
{
  "protocol_version": "v1",
  "node_id": "orhiza_pk_ed25519_01_abcd1234",
  "hardware_fingerprint": "sha256:abcd...",
  "machine_profile": {
    "cpu": {
      "vendor": "GenuineIntel",
      "family": 6,
      "model": 158,
      "stepping": 10,
      "logical_cores": 4
    },
    "memory": {
      "total_bytes": 4294967296
    },
    "network": {
      "mac_addresses": ["52:54:00:12:34:56"]
    },
    "tpm": {
      "present": false,
      "attestation_ready": false
    }
  },
  "devices": [
    {
      "bus_type": "pci",
      "vendor_id": "8086",
      "device_id": "100e",
      "class_code": "02",
      "subclass": "00",
      "prog_if": "00",
      "bus": 0,
      "slot": 3
    }
  ]
}
```

Response:

```json
{
  "success": true,
  "profile_id": "hwprof_001",
  "recognized_devices": 1,
  "unknown_devices": 0
}
```

### `POST /api/v1/driver/query`

Purpose:

- request recommended drivers for current hardware

Request:

```json
{
  "protocol_version": "v1",
  "node_id": "orhiza_pk_ed25519_01_abcd1234",
  "devices": [
    {
      "bus_type": "pci",
      "vendor_id": "8086",
      "device_id": "100e",
      "class_code": "02",
      "subclass": "00",
      "prog_if": "00"
    }
  ]
}
```

Response:

```json
{
  "success": true,
  "recommendations": [
    {
      "match_key": "pci:8086:100e",
      "driver_id": "drv_e1000_native_v1",
      "display_name": "Intel e1000 Native Driver",
      "delivery_type": "builtin_reference",
      "stability_score": 92,
      "performance_score": 88,
      "summary": "Recommended for standard Intel e1000 adapters.",
      "improvements": [
        "Validate RX ring starvation under sustained burst traffic."
      ]
    }
  ]
}
```

### `POST /api/v1/software/query`

Purpose:

- request suggested software packages or tools for the current machine

Request:

```json
{
  "protocol_version": "v1",
  "node_id": "orhiza_pk_ed25519_01_abcd1234",
  "ui_mode": "cli",
  "capabilities": ["network", "storage", "keyboard"]
}
```

Response:

```json
{
  "success": true,
  "packages": [
    {
      "package_id": "pkg_terminal_tools_v1",
      "display_name": "Terminal Starter Tools",
      "summary": "Basic CLI-first package set for networked OpenRhiza systems."
    }
  ]
}
```

### `POST /api/v1/llm/query`

Purpose:

- request available model metadata suitable for the node

Request:

```json
{
  "protocol_version": "v1",
  "node_id": "orhiza_pk_ed25519_01_abcd1234",
  "machine_profile": {
    "cpu": {
      "logical_cores": 4
    },
    "memory": {
      "total_bytes": 4294967296
    }
  },
  "acceleration": {
    "gpu_present": false,
    "npu_present": false
  }
}
```

Response:

```json
{
  "success": true,
  "models": [
    {
      "model_id": "llm_remote_general_v1",
      "display_name": "OpenRhiza Remote General Model",
      "mode": "remote_api",
      "summary": "General-purpose remote inference endpoint for early OpenRhiza nodes."
    }
  ]
}
```

### `POST /api/v1/evaluation/upload`

Purpose:

- upload driver and machine evaluation results back to the service

Request:

```json
{
  "protocol_version": "v1",
  "node_id": "orhiza_pk_ed25519_01_abcd1234",
  "driver_id": "drv_e1000_native_v1",
  "hardware_match_key": "pci:8086:100e",
  "stability_score": 92,
  "performance_score": 88,
  "notes": [
    "Stable during 30 minutes of sustained TCP traffic.",
    "Observed occasional TX backpressure during synthetic burst tests."
  ]
}
```

Response:

```json
{
  "success": true,
  "evaluation_id": "eval_001"
}
```

## v1 Scope Boundaries

Included in v1:

- public-key-based node registration
- machine profile upload
- PCI hardware reporting
- driver recommendations
- software and model catalog recommendations
- evaluation upload

Deferred beyond v1:

- TPM attestation protocol
- binary package transport
- signed driver download manifests
- differential driver updates
- full USB inventory reporting
- browser-like web rendering for the OS

## Implementation Notes for This Repository

The current repository should implement this contract in two stages:

1. `openrhiza-nexus/` exposes route skeletons and static/mock responses using the shapes above.
2. the kernel-side Nexus client evolves from one-off payload fetches into structured JSON calls using the same data model.
