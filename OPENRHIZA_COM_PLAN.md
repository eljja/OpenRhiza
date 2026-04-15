# OpenRhiza.com Integration Plan

This document captures the current plan for connecting the bare-metal OpenRhiza OS to
`openrhiza.com` and for turning the companion `openrhiza-nexus/` project into the first
public-facing service for nodes, hardware knowledge, drivers, software, and model metadata.

## Goals

OpenRhiza should eventually be able to:

- connect to `openrhiza.com` directly over native networking
- identify itself in a stable way across reboots
- describe its hardware inventory in a machine-readable format
- fetch driver recommendations and signed payloads for known hardware
- upload stability and performance feedback for hardware and drivers
- discover software and LLM metadata appropriate for the current machine

The public website should eventually be able to:

- expose machine-oriented API endpoints for OpenRhiza nodes
- expose human-oriented read-only pages for browsing hardware, drivers, software, and node statistics
- accept and organize feedback from nodes about driver quality and hardware behavior

## Architectural Decisions

### 1. Separate node identity from hardware fingerprint

These must not be treated as the same thing.

- `node_id`
  - the stable identity of an OpenRhiza installation
  - derived from a public key
- `hardware_fingerprint`
  - a hash derived from hardware properties
  - used for matching, clustering, and duplicate detection

This avoids tying the installation identity to changeable components such as NICs or storage.

### 2. TPM-backed identity when available

If TPM support exists:

- generate or seal the node private key with TPM
- use the associated public key as the canonical `node_id`
- later allow TPM attestation as a higher-trust enrollment mode

If TPM support does not exist:

- generate a local software keypair
- store it in OpenRhiza-managed storage when persistent storage is ready
- still use the public key as the canonical `node_id`

### 3. Hardware inventory is more important than a single fingerprint

Driver lookup should be based primarily on concrete hardware identifiers, not on a single opaque hash.

The most important identifiers are:

- PCI `vendor_id`, `device_id`, `class_code`, `subclass`, `prog_if`
- later USB `vendor_id`, `product_id`, interface class metadata
- MAC address when needed for node/network identity
- CPU vendor/family/model/stepping
- total memory

### 4. OS-facing APIs come before human-facing pages

The first contract must be an API contract for the OS.

The website UI should be built on top of the same data model and API layer rather than the other way around.

## OS Workstreams

### A. Internet-grade transport

Current repository status:

- native `e1000` path exists
- `smoltcp` integration exists
- a TLS-capable Nexus client exists in `src/https.rs` and `src/tls.rs`

Missing pieces for `openrhiza.com`:

- DHCP client
- DNS resolver
- generalized HTTPS JSON client
- hostname verification and production-grade certificate handling

### B. Node identity and fingerprint generation

Phase 1 target:

- software keypair-backed `node_id`
- `hardware_fingerprint = SHA-256(canonical machine profile)`
- TPM detection field even before TPM-backed enrollment is implemented

Phase 1 machine profile fields:

- CPU vendor/family/model/stepping
- total memory
- native NIC MAC address
- PCI device list
- OpenRhiza version and protocol version

### C. Hardware inventory reporting

Phase 1 target:

- report enumerated PCI hardware
- report active NIC MAC
- report network capability summary
- keep the payload small and text-friendly

### D. Driver and software discovery

Phase 1 target:

- query drivers by hardware identifiers
- query software packages by capability tags
- query LLM metadata by machine capability summary

### E. Evaluation and telemetry upload

Phase 1 target:

- stability score
- basic performance score
- textual improvement notes
- failure signatures and repro hints

## Service Workstreams

### A. API layer

The first service milestone is a small versioned API surface under `/api/v1/`.

First endpoints:

- `POST /api/v1/node/register`
- `POST /api/v1/node/heartbeat`
- `POST /api/v1/hardware/report`
- `POST /api/v1/driver/query`
- `POST /api/v1/evaluation/upload`

### B. Data model

First persisted entities:

- nodes
- node_identities
- hardware_profiles
- hardware_devices
- drivers
- driver_versions
- evaluations
- software_packages
- llm_catalog

### C. Human-facing web

Initial public UI goals:

- browse hardware IDs and known support status
- browse drivers and signed payload metadata
- browse software packages and model catalog entries
- see public aggregate node statistics

## Recommended Implementation Order

1. Freeze the API contract and message formats.
2. Implement service route skeletons in `openrhiza-nexus/`.
3. Add OS-side machine profile and fingerprint generation.
4. Replace single-purpose Nexus fetches with structured JSON API calls.
5. Implement driver query end-to-end.
6. Implement evaluation upload.
7. Build the public read-only web pages on top of the same data model.

## MVP Definition

The first end-to-end milestone is complete when:

- OpenRhiza can reach `openrhiza.com`
- the OS can register a node using a public-key-based identity
- the OS can upload a machine profile and PCI hardware inventory
- the service can return driver recommendations for known hardware IDs
- the public website can show the same inventory and recommendation data in read-only form

## Current Repository Implications

The repository already contains the most important bootstrap pieces:

- kernel-side PCI discovery in `src/arch/x86_64/discovery.rs`
- native MAC extraction in `src/e1000.rs`
- a TLS-capable client path in `src/https.rs` and `src/tls.rs`
- a Next.js companion service in `openrhiza-nexus/`

That means the next step is not starting from zero. The next step is standardizing contracts and
moving from one-off Nexus payload fetches to a stable service protocol.
