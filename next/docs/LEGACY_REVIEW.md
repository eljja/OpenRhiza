# OpenRhiza Legacy Implementation Review

Purpose: reconstruct intended behavior and engineering lessons before a new
implementation. Existing source, binaries, local changes and history are preserved.
This is a thematic repository review plus a reproducible metadata inventory,
not a line-by-line security certification of every file or bundled dependency.

## Review scope

Inventory project Markdown, root build/host tooling, kernel source, sandbox skills
and input drivers, device candidates/simulators, ARM recovery/proof source,
platform/target data, first-party Nexus source/deploy metadata, and verification
tools. Record paths, categories, line counts and SHA-256 hashes in
`legacy-inventory.json` so later work can identify the exact reference state.

The reset inventory contains 364 project source/configuration/document files,
including 76 Markdown documents. Enumeration/hashing covers that corpus; the
code observations below are a thematic path review, not an assertion that every
line of every implementation was audited. Regenerate the inventory if reference
files change; the verification tool detects drift from its recorded hashes.

Generated target trees, `.next`, `node_modules`, `.fslab`, disk/Wasm/WAD/object
files, images/fonts, user logs, secrets and PEM/private keys are not requirements
sources. Third-party libraries/bootloader sources need their own future import
audit; their existence is not a user feature requirement. Do not print credentials
or upload reference material. Curated prior logs are cited, not counted as fresh
tests or re-certified here.

## Documents reconciled by subject

| Subject | Reference set | Decision retained |
| --- | --- | --- |
| Identity/mission | README, VISION, OS, ARCHITECTURE, LEVEL1_BLUEPRINT, HARNESS, ROADMAP | Independent AI-native OS with small core and evolving sandbox capabilities |
| Core/runtime/security | CORE_RUNTIME_FOUNDATION, CORE_BOUNDARY_AUDIT, MODULE_MAP, SECURITY, OPENRHIZA_MODULE_ARCHITECTURE, prior rebuild plan | Strong isolation before feature growth; old permanent-core tags are not carried over |
| Packaging/runtime | CAPABILITY_REGISTRY, HOT_SWAP_RUNTIME, PERSISTENT_DRIVER_CACHE, SANDBOX_MIGRATION_STATUS, QEMU_DRIVER_SET, BOOT_AUTORUN | Cache/registry/generation order, live/persisted split, health and rollback |
| Service ecosystem | OPENRHIZA_COM_PLAN, OPENRHIZA_API_V1, Nexus README/CI_CD/source | Shared capability/evidence registry plus separate forge; API draft not frozen |
| Desktop | GUI_DEVELOPMENT, DISPLAY_ABI, WINDOW_PROTOCOL, DEFAULT_DESKTOP_APPS, DESKTOP_ACTIONS, OPENRHIZA_DESKTOP_COMPLETION | Command-first, inspectable GUI, real multiwindow/apps; old completed labels reset |
| Input/text | SANDBOX_INPUT_DRIVER, USB_HID_HANDOFF, FONT_SKILL, VOICE_INPUT, Hangul/font/voice tools | Device/text/voice modules, focus and privacy, Korean/English and session ergonomics |
| Storage/knowledge | FILESYSTEM_SKILLS, FILESYSTEM_VALIDATION_MATRIX, STORAGE_HOST_ABI, STORAGE_IMAGE_HARNESS, SKILL_FS_BRIDGE, all filesystem and semantic object docs | Guest files/durability plus rebuildable private semantic overlay; host labs not guest support |
| Autonomy/development | AUTONOMY_MODE, GUI_DEVELOPMENT, compiler/browser/store/security/forensics prototypes | Controlled planning and self-hosting; inspectable real tools, not scripts pretending to execute |
| Applications/performance | DOOM_SKILL, DOOM_MODULE_ACCEPTANCE, Doom port notes, SANDBOX_EXECUTION_ACCELERATION | Real game as integration gate; no game-specific core or starvation for FPS |
| Platforms/compatibility | PLATFORM_EXPANSION, PLATFORM_PROGRESS, CROSS_PLATFORM_BRINGUP, platform README/matrix/targets, PROGRAM_COMPATIBILITY_GOALS | Architecture-independent contracts; expansion and compatibility separately gated |
| Quality/publication | BUILD_AND_TEST, VALIDATION_MATRIX, PUBLIC_ALPHA_CHECKLIST, PUBLIC_READINESS, KNOWN_ISSUES, CONTRIBUTING, CHANGELOG, PROTOCOL, DOC_INDEX, Codex | Honest measured claims, reproducibility, failure/recovery and start/end progress reports |
| Historical suggestions | Gemini readiness/walkthrough, sandbox analysis | Useful bootstrap/physical-risk questions; promotional claims and product pivots not authoritative |

## Code lessons that change the new design

1. `src/main.rs`, `src/display.rs`, `src/task/keyboard.rs` and
   `src/core/seed.rs` combine orchestration, UI, model/API work and device brokers
   with kernel execution. Fresh kernel ownership starts with mechanisms, not a
   refactor of these files. Hundreds of host imports are not a minimal syscall ABI.
2. `src/task/executor.rs` is cooperative; spawn can panic on a full queue.
   `src/arch/x86_64/interrupts.rs` page faults loop, and the kernel panic path also
   loops. Round-robin poll counts do not establish preemption or fault containment.
3. Wasmi executes in kernel context. Module-provided packet lengths can cause
   kernel allocations before validating guest memory. Global handle/device state,
   broad host imports and DMA/MMIO operations do not provide principal-isolated
   ownership or enforced IOMMU containment. These paths are not copied.
4. The generic driver download path records active/preferred bindings and payload
   notes without compiling/loading that payload and proving device cutover.
   Separate e1000/PIIX/xHCI experiments contain useful observations, but do not
   establish the generic lifecycle or a hardware-isolated driver framework.
5. TLS skips peer authentication steps; unsigned skill delivery can be cached and
   loaded. A mock binary/header-signature route and v1 JSON delivery are different
   paths. New transport, manifest admission and tests fail closed independently.
6. `build.rs` loads a model API key from environment/local `.env` and emits it as
   a compile-time value consumed by `src/api_v1.rs`. New images/packages never
   carry user API keys; credentials are separately protected service objects.
7. `src/main.rs` foreground-frame branch skips normal network/driver/API/GUI work.
   Doom's port timing, no-op write shims and audio/input gaps make presentation FPS
   insufficient playability evidence. New scheduling preserves all admitted
   budgets and separates simulation time from presentation time.
8. Desktop windows are static scene objects, not independent applications.
   Browser navigation maps URLs to hard-coded HTML; its JS handler is string
   matching. App/Nexus stores, VFS, firewall and memory-forensics seeds operate on
   local/static mock data. Preserve UX ideas, reject fake installation, storage,
   network filtering, process permissions or kernel-memory access claims.
9. `nic_drivers/driver_manifest.json` contains 20 candidates. Python simulator
   tests generally exercise Python models rather than compiled Rust artifacts;
   `test_qemu_hw.py` includes unconditional true assertions. Scores and source
   files are not guest device support. Require real I/O and independent fault tests.
10. Filesystem family read/write tests use WSL filesystems; guest probes identify
    signatures and scratch blocks. Those are valuable fixtures, not guest VFS
    implementations. The host semantic indexer similarly does not establish
    guest indexing/query or permission-aware context retrieval.
11. ARM has a separate recovery kernel and opt-in Wasmi/proof harness. The large
    proof/counter path is not the x86 userspace runtime on ARM. New architecture
    parity comes from a shared native ABI/module model with fresh platform gates.
12. Nexus has actual SQLite storage, public boards, uploads/downloads and a
    configured Gemini proxy, but lacks a complete authenticated, immutable,
    provenance/evidence-bound release pipeline. Claimed identity/TPM trust,
    self-supplied scores and votes are not independent evidence. A deploy workflow
    alone is not kernel/module/security CI or tested production rollback.

Portable ideas to study include CPU entry/serial diagnostics, device register and
descriptor observations, bounded storage scratch fixtures, GUI interaction cases,
Hangul composition cases, graph schemas, existing game portability work, and
explicit negative-test requirements. Reimplement from specifications/contracts
and validate independently; do not copy a successful-looking path wholesale.

## Conflicts resolved

- The user's new-start request supersedes the earlier "use the current kernel as
  starting point" decision and all migration-oriented core tags.
- `OPENRHIZA_DESKTOP_COMPLETION.md` marks all six old desktop milestones complete
  using scene labels/selftests. None is a passed new OS stage.
- Some earlier integration docs said DNS was absent; actual `src/dns.rs` and QEMU
  logs show native A-query resolution. An earlier overview described TLS as
  dormant while the API path used it. Source/path evidence outranks stale text.
- Doom performance notes and later failing logs disagree. No historical best
  number is transferred; the new playability gate uses correct time, input,
  sound, durable saves and background behavior.
- Recovery cannot mean keeping large network/storage/Wasm/UI implementations in
  core. Supply an offline signed module set and reserve recovery execution instead.
- A model council or popular registry artifact cannot authorize itself. Human
  policy and independent admission are retained, not zero-supervision promises.

## Reference verification for this reset

Legacy branch: `codex/sandbox-core-migration`; tracked/untracked work is already
dirty and has not been discarded. The reference tool versions observed are
Rust `1.96.0-nightly` (2026-03-21 commit) and QEMU `10.2.0`. These are observations,
not the new toolchain pin. Root Cargo and patched bootloader settings are legacy.

The existing x86_64 TCG image was exercised with `/status`, `/scheduler-status`,
`/wasm-status`, `/desktop-status`, `/platform-status`, `/recovery`; see
`../../logs/x86-legacy-reference-reset-review-20261003.log`. It is a legacy
boot/CLI/recovery baseline only, not new kernel or malicious-module survival
evidence. The image was not rebuilt for this documentation/reset work.
The command harness exited successfully, and the reviewed serial log contains
command responses and recovery selection with no panic/page-fault/triple-fault
markers. This short run did not test graphical interaction, game playability,
power-loss integrity, concurrent protected applications or hostile modules.

The immediately preceding ecosystem review recorded public board/health HTTP
200, guest DNS/registry queries and Nexus lint (zero errors, two warnings).
Those results are prior evidence, not rerun supply-chain/security tests. No
public upload/deployment, paid model job, key rotation or old source deletion is
part of this reset.
