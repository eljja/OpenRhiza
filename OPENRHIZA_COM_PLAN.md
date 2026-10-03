# OpenRhiza.com Integration Plan

Reference investigation after the 2026-10-03 new-implementation reset. The OS,
Nexus and forge goals below informed `next/docs/DIRECTION.md` and
`next/docs/REQUIREMENTS.md`; the active sequence is `next/docs/ROADMAP.md`.
Neither the existing Nexus API nor its artifact/trust model is inherited as a
production contract. Earlier live-site checks are historical observations.

This document captures the current plan for connecting the bare-metal OpenRhiza OS to
`openrhiza.com` and for turning the companion `openrhiza-nexus/` project into the first
public-facing capability registry for nodes, hardware knowledge, drivers, programs, skills, workflows, policies, evaluations, software, and model metadata.

## Ecosystem re-audit: 2026-10-03

This review covers the independent OS, the Next.js Nexus service, the public
website, earlier host AI/mock-server tools, device candidates, and long-term
self-hosting goals. It does not complete Stage 1 of `OPENRHIZA_REBUILD_PLAN.md`.
`OPENRHIZA_MODULE_ARCHITECTURE.md` governs the target kernel/module boundary;
older references below to kernel-side networking or sandbox execution describe
the bootstrap implementation, not permanent kernel responsibilities.

### Intended product, reconstructed from the repository

OpenRhiza is one ecosystem with three cooperating parts:

1. **Independent local OS:** a minimal survival/isolation kernel plus replaceable,
   sandboxed drivers, services, input pipelines, desktop, applications, runtimes,
   planners, and policies. Installed capabilities remain usable offline.
2. **OpenRhiza.com / Nexus:** shared operational memory for capability discovery,
   immutable artifacts, hardware/platform compatibility, prior failures, execution
   evidence, model endpoints, comments, votes, and release history. Its human
   boards and machine API should describe the same records.
3. **Module forge and validation:** acquire existing work first; otherwise use an
   LLM to propose source, independently build and test it, authorize publication,
   sign an approved artifact, and allow bounded local canary activation/rollback.

Nexus can be hosted using conventional server infrastructure without making
OpenRhiza itself a Linux-hosted OS. The guest must own its execution, drivers,
services, desktop, and recovery; cloud availability must not be required to boot
or operate already-installed modules. Self-hosting Nexus on OpenRhiza may be a
future milestone, but is not currently implemented or required for first alpha.

The wider intended capability graph also includes:

- command-first boot and one intent path for typed, pointer, and confirmed voice
  input; direct registered commands do not require model inference;
- real processes, background jobs, multiwindow desktop, workspaces, and
  application-owned MDI/document surfaces, rather than a fixed set of labels;
- self-hosted development and object-scoped GUI inspection/mutation;
- block/VFS/filesystem capabilities with integrity and power-loss recovery;
- a private, rebuildable semantic sidecar graph for relevant local file context;
- opt-in assist/council planning, evidence gathering, approval, cancellation,
  auditing, and undo; council votes are advisory, not authorization;
- downloadable Doom engine and separately licensed content modules as a real
  application test of input, timing, graphics, audio, persistence, and isolation;
- modular foreign-program loaders, ABI adapters, and runtimes, deferred until
  native contracts are stable;
- per-architecture/device evaluation, progressing from x86_64 QEMU to ARM64
  QEMU, a qualified board, and only later a specific unlocked phone.

### Observed implementation versus intended capability

| Area | What exists | What has not been established |
| --- | --- | --- |
| OS kernel | Bare-metal x86_64 boot, CLI/recovery, cooperative tasks, in-kernel Wasmi | Preemptive isolated processes, user-mode Wasm, module-fault core survival, real SMP |
| Network | e1000/smoltcp, UDP DNS A-query client, general JSON HTTP/TLS client | DHCP/general configuration, authenticated production TLS, restartable user-mode stack |
| Node identity | Machine inventory/fingerprint and per-boot provisional random identity | Durable signing keypair, proof of key possession, verified TPM attestation |
| Nexus data | SQLite WAL tables for capabilities, artifacts, nodes, evaluations, driver comments/votes | Unified versioned module/dependency contracts, immutable release channels and evidence bindings |
| Public web | Eight board routes and driver/program/model/node detail routes | Authenticated contribution/review/release operations and privacy-aware public projections |
| Model access | Guest Gemini client and server Gemini text proxy when configured | Provider-neutral bounded jobs, typed planner enforcement outside kernel, local inference |
| Driver acquisition | Source-text/base64 storage, download, binding bookkeeping; separate device-specific experiments | A generic downloaded-code build/load/device-cutover/health/commit transaction |
| Skill acquisition | Wasm hex download, fixed-slot cache, load/run; optional signature handling | Mandatory verified manifest/digest/signature, grants, revocation and rollback protection |
| Generation | Host `host_brain.py` can ask Gemini, compile with Cargo, inject through serial and retry | Hermetic forge with independently controlled tests and release signer; fully in-OS toolchain |
| Development prototypes | A micro-Rust lexer/parser/Wasm emitter and seed GUI tools | A general Rust/C compiler/SDK and complete self-hosted OS development |
| Device inventory | 20 candidate records in `nic_drivers/driver_manifest.json`, Rust files and Python simulations | Qualification of those Rust artifacts as working guest/device drivers |
| Desktop/voice | GUI object/skill prototypes, static desktop seed, stored voice routing and host bridge | Independent interactive apps, general window lifecycle, guest microphone/ASR/audio chain |
| Semantic storage | Host graph builder/query tool; guest status/introspection | In-guest index generation/query and permission-aware automatic refresh |
| Compatibility/platforms | Design documents, target matrix, separate ARM64 recovery/proof harnesses | General foreign binaries, full ARM64 runtime parity, production phone support |
| Operations | SSH deploy workflow, systemd sample, standalone Docker build | Tested backup/restore, explicit migrations, staged deploy/rollback, release identity and security CI |

Evidence anchors: `src/identity.rs`, `src/dns.rs`, `src/tls.rs`,
`src/main.rs`, `src/driver_runtime.rs`, `src/skill_cache.rs`,
`openrhiza-nexus/src/app/registry-data.ts`,
`openrhiza-nexus/src/lib/openrhiza-v1.ts`, `host_brain.py`,
`register_nic_drivers.py`, `nic_drivers/tests/`,
`sandbox-skills/rhiza_compiler_seed/src/lib.rs`, and
`tools/semantic_graph_layer.py`.

### Public-release blockers identified by source review

These are local-source findings, not an exploit test of the public deployment.
The deployed commit/configuration has not been identified.

1. **Unauthenticated contribution and model spending:** reviewed write routes and
   the Gemini generate route accept JSON without caller authentication or
   per-principal authorization. TypeScript casts and a few required-field checks
   are not complete runtime validation. Add schema/body/output limits, quotas,
   rate limits, bounded model deadlines, ownership checks, and moderation before
   exposing these as a trusted public supply chain.
2. **Unverified identity:** `registerNode` can replace a node's public key by ID
   and assigns TPM trust from the claimed `identity_type`, not attestation.
   Guest identity is provisional random bytes, not a persistent signing keypair.
   Separate enrollment, challenge signatures, key rotation and verified trust.
3. **Unsigned mutable delivery:** skill artifact uploads can overwrite an existing
   artifact ID; downloads select the latest artifact without an approved release
   channel. The service does not provide a signed module manifest. The guest's
   SkillDownload path verifies a signature only if one is supplied, then can
   cache/load/run an unsigned payload. Verified-cache preference is not a
   signature-required admission policy.
4. **Unauthenticated TLS peer:** `src/tls.rs` explicitly skips server Finished
   verification and CertificateVerify verification; certificate-chain/hostname
   validation is not established. Encryption/HTTP 200 does not prove server
   authenticity. Move audited TLS into a module and test rejection paths.
5. **Bookkeeping can look like real activation:** the generic DriverDownload path
   calls `activate_binding`/`promote_binding` and stores payload text without
   compiling/loading that payload and proving hardware cutover. Do not confuse
   this path with separate e1000/storage/input cutover experiments.
6. **Evidence and ranking are not trust:** scores can be seeded or caller-supplied;
   driver recommendations sort vote score first and votes have no demonstrated
   authenticated, unique-voter constraint. Some `test_qemu_hw.py` checks assert
   literal `True`. Other NIC tests run Python device simulations, not the Rust
   artifact. Host/model/mock results must not promote guest/hardware support.
7. **Development trust material:** tracked `nexus_key.pem` and deterministic
   `mock_signer.py` signing material must remain explicitly test-only and must
   never authorize a public release. Production key provenance, rotation and
   revocation require their own gate; this review did not rotate any live key.

The earlier `mock_nexus_server.py` and `/api/nexus/*.wasm` header-signature route
are a different transport from Nexus v1 JSON downloads. Their successful mock
signature check does not establish signed delivery on the public v1 path.
`driver_nexus_client_seed` also contains a hard-coded catalog and simulated
installation messages; those are not cloud downloads or hardware support.

### Correct end-to-end lifecycle

```text
intent + local capabilities
  -> local approved cache -> registry evidence/compatibility lookup
  -> reuse candidate OR bounded LLM source-generation job
  -> pinned isolated build + source/dependency/license/provenance records
  -> independent contract/adversarial tests + disposable QEMU device tests
  -> approved immutable digest + signed manifest + release channel
  -> local policy/grant admission + sandbox canary
  -> quiesce/state handoff + health verification + atomic active commit
  -> persist approved preference OR revoke candidate and roll back
  -> consented, redacted evaluation for that exact artifact/platform
```

The generator cannot edit the authoritative tests, grant itself capabilities,
choose production signing authority, or bypass local policy. An LLM's analysis
can propose repairs and summarize failures; it cannot vote a module into safety.
Hardware/ABI matching, mandatory tests and revocation are eligibility gates;
votes and measured performance only rank candidates that already pass them.

Local semantic content, voice recordings, prompts, keys, raw MAC addresses and
machine fingerprints are not public telemetry by default. Publish only a
policy-approved projection; keep private workspaces and their permissions apart
from shared operational evidence. The registry is advisory data, not kernel
authority, and a compromised/unavailable registry must not disable recovery.

### Verification performed in this review

- Read-only public checks: `/`, `/api/health`, `/drivers`, `/skills`, `/workflows`,
  `/policies`, `/software`, `/models`, `/nodes`, `/evaluations` returned HTTP 200.
  Health reports a Nexus handler timestamp, not deployed commit, DB integrity,
  signing health, artifact integrity or successful module execution.
- `npm run lint` in `openrhiza-nexus` passed with zero errors and two unused-input
  warnings in software/model queries. Those queries return the entire catalog,
  not a demonstrated machine-specific recommendation.
- Existing x86_64 QEMU TCG image booted with e1000, xHCI keyboard/mouse and the
  FAT driver/cache disk. `/status`, `/registry-context`, `/skill-cache`,
  `/driver-runtime-status`, `/semantic-status`, `/autonomy-status`,
  `/voice-status`, `/recovery` all reached their handlers.
- `logs/x86-ecosystem-review-20261003.log` also records native DNS resolution and
  public skill/workflow queries returning HTTP 200 during existing boot autorun.
  It reports seven loaded Wasm modules at semantic inspection, no storage harness,
  autonomy off, and voice capture requiring a sandbox skill.
- This run did not test module forge, server writes, paid generation, certificate
  rejection, signatures, adversarial isolation, interactive desktop, Doom or
  power-loss recovery. No runtime source was changed or image rebuilt here; the
  immediately preceding `cargo bootimage` baseline was reused. The QEMU smoke
  passed, but does not permit skipping Stage 1 or claiming core survival.

### How this fits the six-stage rebuild

| Rebuild stage | OS/Nexus joint deliverable |
| --- | --- |
| 1 | Freeze shared manifest/API/evidence contracts; inventory test keys/mock claims; pin builds and add reproducible CI. |
| 2 | Prove process/Wasm containment and capability admission locally, without trusting a remote service. |
| 3 | Implement secure enrollment, authenticated TLS, immutable signed delivery, local cache and actual restartable driver/service cutover. |
| 4 | Run real desktop/app modules and expose module/input/window lifecycle as measurable actions. |
| 5 | Complete the independently tested forge, typed intent/policy/undo, voice, semantic capabilities and downloadable Doom. |
| 6 | Qualify public contribution/release roles, abuse controls, revocation, privacy, backups, update rollback and supported hardware. |

Authentication, signature enforcement and TLS peer verification are prerequisites
to Stage 3 delivery, not work to postpone until Stage 6. Stage 6 qualifies their
public operation. Broad compatibility, local inference, accelerated vendor GPUs,
Wi-Fi, full ARM64 parity and phones remain separate gated expansion programs.

## Goals

OpenRhiza should eventually be able to:

- connect to `openrhiza.com` directly over native networking
- identify itself in a stable way across reboots
- describe its hardware inventory in a machine-readable format
- fetch driver recommendations and signed payloads for known hardware
- fetch skills and workflows for display, GUI, filesystem, testing, and orchestration tasks
- fetch policies that constrain safe activation, rollback, and autonomy behavior
- upload stability and performance feedback for hardware and drivers
- discover software and LLM metadata appropriate for the current machine
- upload evaluations for drivers, skills, workflows, policies, and generated artifacts

The public website should eventually be able to:

- expose machine-oriented API endpoints for OpenRhiza nodes
- expose human-oriented read-only pages for browsing hardware, drivers, skills, workflows, policies, software, and node statistics
- accept and organize feedback from nodes about capability quality and hardware behavior

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
- generic HTTP/JSON and bootstrap TLS clients exist in `src/https.rs` and `src/tls.rs`
- a native UDP DNS A-query client exists in `src/dns.rs` and was exercised in QEMU

Missing pieces for `openrhiza.com`:

- DHCP client
- broader DNS/configuration handling beyond the QEMU A-query bootstrap
- moving the network/HTTP/DNS/TLS clients out of the kernel into restartable modules
- server Finished, CertificateVerify, chain and hostname verification

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

### D2. Capability discovery

Phase 1 target:

- query skills by capability tag and runtime requirement
- query workflows by user intent and available local capabilities
- query policies by action domain, such as storage write, runtime hot-swap, autonomy, and GUI mutation
- prefer known-good registry capabilities before asking the LLM to generate a new one

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
- `POST /api/v1/skill/query`
- `POST /api/v1/workflow/query`
- `POST /api/v1/policy/query`
- `POST /api/v1/evaluation/upload`

### B. Data model

First persisted entities:

- nodes
- node_identities
- hardware_profiles
- hardware_devices
- drivers
- driver_versions
- skills
- workflows
- policies
- evaluations
- software_packages
- llm_catalog

### C. Human-facing web

Initial public UI goals:

- browse hardware IDs and known support status
- browse drivers, skills, workflows, policies, and signed payload metadata
- browse software packages and model catalog entries
- see public aggregate node statistics

## Recommended Implementation Order

1. Freeze the API contract and message formats.
2. Implement service route skeletons in `openrhiza-nexus/`.
3. Add OS-side machine profile and fingerprint generation.
4. Replace single-purpose Nexus fetches with structured JSON API calls.
5. Implement driver, skill, workflow, and policy query end-to-end.
6. Implement evaluation upload for all capability types.
7. Build the public read-only web pages on top of the same data model.

## MVP Definition

The first end-to-end milestone is complete when:

- OpenRhiza can reach `openrhiza.com`
- the OS can register a node using a public-key-based identity
- the OS can upload a machine profile and PCI hardware inventory
- the service can return driver recommendations for known hardware IDs
- the service can return skill, workflow, and policy recommendations for known capability requests
- the public website can show the same inventory and recommendation data in read-only form

## Current Repository Implications

The repository already contains the most important bootstrap pieces:

- kernel-side PCI discovery in `src/arch/x86_64/discovery.rs`
- native MAC extraction in `src/e1000.rs`
- a TLS-capable client path in `src/https.rs` and `src/tls.rs`
- a Next.js companion service in `openrhiza-nexus/`

That means the next step is not starting from zero. The next step is standardizing contracts and
moving from one-off Nexus payload fetches to a stable service protocol.
