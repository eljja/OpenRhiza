# Astra Plan Review Disposition

Date: 2026-10-03. Status: initial review integrated; Astra plan re-review complete.

The user requested an Astra review before kernel implementation and Astra for
the subsequent kernel work. The selected reviewer is `gpt-6-astra`, reasoning
effort `max`, agent `01a0ff02-15af-7270-b1dd-37dbd7cbf541`.
Findings belong to `ASTRA_PLAN_REVIEW.md`; this file records their disposition
and the follow-up review of the updated plan. A model review is engineering
evidence, not a proof of defect-free implementation.

## Initial Review Inputs

These SHA-256 values identify the documents at dispatch, before review changes.
Paths are relative to `next/`.

| Document | SHA-256 |
| --- | --- |
| `README.md` | `b224cf3d8b9f76679143308e1d152ae9e50243de6a23cc83b738c7a26b2fc348` |
| `docs/DIRECTION.md` | `52049a58b8344f3ee9cf70ba90313d4108980d6d569a3bb6fa2c3d899a6790e1` |
| `docs/REQUIREMENTS.md` | `2099b7bc15f3a2fa7df4a706f7e7131d16c11ccdc7b29def11c4a38ee01cfd34` |
| `docs/ROADMAP.md` | `f71bd9be0ef936e2ad266f29233a789756e76321ae2e03f2d8bae0b58bd671ba` |
| `docs/LEGACY_REVIEW.md` | `f9d2e559e0428f68c6ce3ba7433b07931b50fb139416732b949a844b41efc53b` |

## Verification Context

Before integration, `tools/verify-baseline.ps1` passed against the R0 reference
inventory. Installed host tooling includes Rust stable/nightly and QEMU
`10.2.0 (v10.2.0-12105-g0f12d445bd)` with EDK2 x86_64 firmware files. Availability
does not qualify a new toolchain, firmware, machine profile or kernel image.

The existing x86_64 image passed the `/status` and `/recovery` command smoke using
the legacy TCG harness. Its serial log contains both commands and recovery
selection, with no panic/page-fault/triple-fault markers in the reviewed output.
Evidence: `../../logs/x86-astra-plan-review-reference-20261003.log`.
This remains old-image reference evidence. There is no new kernel to boot yet.

## Disposition And Readiness

The initial review found eight P1 and two P2 plan-contract gaps, with no P0/P3
asserted. All ten are incorporated as decisions and evidence requirements in
`DIRECTION.md`, `IMPLEMENTATION_GATES.md` and `ROADMAP.md`. This closes neither
their concrete implementation nor their future gate. The original report's
line references address the hashed pre-edit snapshots above; current requirements
are in the amended canonical documents.

| Finding | Plan change | Remaining closure |
| --- | --- | --- |
| F01 | K requires external boot trust, bounded handoff, memory/parser ownership and first authority | Select/review K ADRs and fixtures before Stage 1.2; verify image admission before initial module launch |
| F02 | K/N define resident recovery dependencies, protected reservations, restart limits and bounded degraded behavior | Choose initial console/authority path at K; enforce and fault-test recovery at N |
| F03 | N defines payers, metadata/delegation charges, logical revocation vs physical reclaim and completion | Concrete ledger/state machine and race/reuse evidence before hostile CPU-only qualification; extend before SMP |
| F04 | N specifies syscall/lifecycle outcomes, immutable snapshots, bounded request/reply and inversion rules | Draft first ABI at K; implement/test first-operation semantics at N |
| F05 | D gates generic DMA/MMIO/IRQ grants and quarantine; narrow initial non-DMA console is separately qualified | Choose required profile at K; prove D before generic grants or hostile-driver claims |
| F06 | A places time/entropy, local authorization freshness/security epochs and offline rollback policy before external admission | Fixed-image trust at N; persisted trust-state and failure tests in Stage 3 before enrollment/install |
| F07 | A requires fencing, epochs, active/persisted commit points and an allowed-crash-state oracle | Concrete service/storage protocol before real cutover or writable user data |
| F08 | K defines oracle ownership; acceptance sheets freeze limits/deadlines and use broken negative controls | Qualify the harness at K/1.3 and calibrate each gate before its acceptance run |
| F09 | Stage 2 introduces local typed commands; Intent binds approval to principal/object/version/commit | Enforce local subset at N; test approval/replay/TOCTOU before Stage 5 effects |
| F10 | Boot envelope first, experimental wider ABI; native/Wasm/SMP subgates | Qualify operational subsets before freezing; allow only explicitly single-vCPU progress until SMP passes |

The Stage 3 wording explicitly holds external enrollment/install until minimum
trust-state persistence/recovery is qualified. Network protocol development can
precede that point without claiming authenticated durable node identity.

## Astra Re-review Result

The same `gpt-6-astra` reviewer accepted the amended plan for **Stage 1.1 K
contract work** and found no blocking integration contradiction. The report's
appended re-review verifies F01-F10 incorporation and the initial-console,
durable-admission and single-vCPU/Wasm/SMP sequencing. This is plan-level
acceptance, not Stage 1.2 entry authorization or an implemented guarantee.

Astra identified one nonblocking P2 timing-wording drift: reclamation was measured
after "acknowledged teardown" rather than stop/revoke request acceptance.
The parent applied the requested correction in `IMPLEMENTATION_GATES.md` after
re-review: define protected acceptance before asynchronous teardown, record
submission/acceptance/baseline separately, and require a numeric request-latency
bound plus a host end-to-end timeout. Final teardown acknowledgment cannot move
the start of the measured interval. These are calibration requirements, not
measured timings. The report's re-reviewed hashes precede this directed correction
and this disposition update; the historical snapshots remain unchanged.

Post-integration inventory regeneration and `tools/verify-baseline.ps1` passed
for 364 reference files including 76 project documents. The touched root handoff
documents passed `git diff --check`. QEMU evidence remains the existing-image
command/recovery smoke described above; no new image was built or executed.

Stage 1.1 work may proceed, but its K choices/fixtures/entry review have not been
performed. Stage 1.2 kernel code, hostile native/device execution and public
release remain unqualified. A later review of the actual K artifacts is distinct
from re-reviewing this amended plan. Kernel implementation will use the requested
Astra model, with build, QEMU, fault injection and repair inside each stage.
