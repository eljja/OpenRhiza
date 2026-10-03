# OpenRhiza Requirements and Source Traceability

This register preserves the user's intent without importing legacy architecture
or implementation-completion claims. Direct user requirements take precedence
over old documents. Repeated repository goals are supporting evidence; isolated
AI suggestions are proposals, not new human requirements.

All requirements below are unimplemented in the new runtime. R0 completes their
collection/architecture baseline only. Detailed ABI fields/test tolerances become
versioned acceptance contracts as their stages begin.

The Astra review's `IMPLEMENTATION_GATES.md` specifies where these contracts
must be selected, enforced and tested. Its numerical calibration candidates are
not measured results; freeze profile-specific thresholds before acceptance runs.
Plan-review closure does not imply that implementation or entry gates passed.

## Product requirements

| ID | Requirement | Source of intent | New acceptance boundary |
| --- | --- | --- | --- |
| R01 | Independent new OS, no existing OS kernel/runtime underneath | User; `VISION.md`, `ARCHITECTURE.md`, `OPENRHIZA_REBUILD_PLAN.md` | New source/build/image boots without another OS; host tools are development-only |
| R02 | Minimal core; all other behavior is a sandbox module | User; `OS.md`, `CORE_BOUNDARY_AUDIT.md`, `HOT_SWAP_RUNTIME.md` | Kernel ownership/dependency audit; no permanent feature fallbacks |
| R03 | Module failures cannot crash/starve the core | User; `SECURITY.md`, `OPENRHIZA_MODULE_ARCHITECTURE.md` | Declared fault assumptions, same-boot fault tests, independent witness and resource recovery |
| R04 | Modules are acquired from Nexus or generated with capable LLMs | User; `CAPABILITY_REGISTRY.md`, `OPENRHIZA_COM_PLAN.md` | Local reuse, authenticated discovery, independent build/test/sign/admit pipeline |
| R05 | Boot to commands; optional GUI/desktop | User; `BOOT_AUTORUN.md`, `PUBLIC_READINESS.md` | Offline user terminal plus independent recoverable GUI launch |
| R06 | Direct commands execute locally; other intent uses LLM planning | User; `OS.md`, `DESKTOP_ACTIONS.md`, prompt/API sources | Typed action registry and external policy/transaction/cancellation checks |
| R07 | Keyboard/mouse first, then optional voice | User; `VOICE_INPUT.md`, `SANDBOX_INPUT_DRIVER.md`, `USB_HID_HANDOFF.md` | Focused input; visible bounded capture and confirmed transcript through one router |
| R08 | Real multitasking, background execution, MDI and multiple windows | User; `WINDOW_PROTOCOL.md`, `DEFAULT_DESKTOP_APPS.md` | Preemptive isolated apps/jobs, real surfaces/documents and input-driven lifecycle tests |
| R09 | Genuinely playable downloadable Doom module | User; `DOOM_MODULE_ACCEPTANCE.md`, both Doom port sources | Correct game clock, input, pixels, sound, durable saves, offline launch and contained faults |
| R10 | OpenRhiza.com as shared capability knowledge | User; `README.md`, `CAPABILITY_REGISTRY.md`, Nexus routes/schema | Human/machine projections of versioned artifacts, platforms, workflows, policy, evaluations and history |
| R11 | Stable node identity and hardware-aware selection | `OPENRHIZA_COM_PLAN.md`, `OPENRHIZA_API_V1.md`, `src/identity.rs` | Durable keypair/proof of possession; inventory separate from identity; verified trust |
| R12 | Registry and field feedback improve reusable work | `README.md`, `register_nic_drivers.py`, Nexus comments/votes | Exact artifact/platform/environment evidence; anti-abuse; votes never certify safety |
| R13 | Live replace/rollback, independent persistence preference | `HOT_SWAP_RUNTIME.md`, `PERSISTENT_DRIVER_CACHE.md`, runtime sources | Quiesce/state transfer/atomic commit/health/rollback, including device revocation/reset |
| R14 | Offline/cache-first behavior and network-independent recovery | `OS.md`, `PERSISTENT_DRIVER_CACHE.md`, NIC readiness analysis | Signed offline initial set/cache; registry/model outage tests |
| R15 | Safe real storage and filesystem interoperability | `FILESYSTEM_SKILLS.md`, `STORAGE_HOST_ABI.md`, all `filesystem-skills/` docs | VFS/filesystem modules; guest file operations and crash/power-loss durability, not WSL scores |
| R16 | Private semantic knowledge of managed files | `SEMANTIC_GRAPH_LAYER.md`, `semantic-graph/`, host graph source | Permission-aware bounded indexing/query/refresh; raw content stays private by default |
| R17 | Self-hosted development and GUI improvement | `VISION.md`, `GUI_DEVELOPMENT.md`, compiler/font/scene prototypes | In-guest SDK/build/validation and reversible object actions; not toy parser equivalence |
| R18 | Opt-in bounded autonomy and council proposals | `AUTONOMY_MODE.md`, `src/autonomy.rs` | User controls modes/budgets; evidence/approval/timeout/cancellation, no vote-based authorization |
| R19 | Korean/English text, IME and usable long sessions | `PLATFORM_PROGRESS.md`, `FONT_SKILL.md`, `src/hangul.rs`, GUI docs | Text/composition/selection/wrapping/history/input tests through module services |
| R20 | Useful native desktop applications and developer tools | GUI/default app docs; VFS/store/browser/security/forensics seeds | Real service-backed behavior and permissions, not static catalogs or fake process/memory data |
| R21 | Remote/local model choices and later acceleration | `HARNESS.md`, model catalog, `SANDBOX_EXECUTION_ACCELERATION.md` | Replaceable provider modules; bounded secret/model access; local acceleration separately qualified |
| R22 | Cross-platform expansion without kernel feature growth | `PLATFORM_EXPANSION.md`, `PLATFORM_PROGRESS.md`, target matrix | Common ABI/user contracts; x86 reference, ARM QEMU/board, RISC-V, then qualified phone |
| R23 | Foreign-program and desktop interoperability | `PROGRAM_COMPATIBILITY_GOALS.md`, GUI/rebuild plans | Optional loader/ABI/runtime/Wayland adapters in modules; not importing Linux as the OS |
| R24 | Trustworthy updates, distribution and public release | User; `PUBLIC_ALPHA_CHECKLIST.md`, `SECURITY.md`, `BUILD_AND_TEST.md` | Signed immutable releases, revocation, A/B rollback, consent, SBOM/licenses and honest scope |
| R25 | Reproducible development and explicit progress/QEMU reports | User; `HARNESS.md`, `CONTRIBUTING.md`, validation/build docs | Independent clean build; versioned fixtures and per-stage QEMU gates; start/end checkpoint |

## Evidence categories

Use distinct labels: design, inventory, host model, host library/lab, guest
execution, emulated-device execution, physical hardware, and adversarial/soak.
Attach source/binary digest, build/tool versions, configuration, test revision,
input, measured result, failure and recovery behavior to each claim.

The manifest/signature proves artifact identity and authorization, not functional
correctness. A simulator validates its model, not automatically a Rust driver.
Guest code must be invoked through production paths; changing a test-only export
or a state flag does not establish real service behavior.

## Intent not adopted as a requirement

- Old analyses propose a B2B sandbox product or an edge/IoT pivot. Those can be
  extensions, but the human request remains an independent general-purpose OS.
- "Zero-human-in-the-loop" research text does not override explicit consent,
  privacy, autonomy-off defaults or deterministic authorization.
- Automatic LLM/telemetry voting is discovery/diagnostic context, not safety proof.
- In-kernel Wasm, native "graduation", fixed FAT slots, global object numbers,
  specific Gemini models and ad-hoc JS/HTML parsers are old implementation choices.
- KDE/GNOME compatibility is deferred; neither becomes the initial kernel or
  desktop dependency. Porting standards/components later is distinct from
  replacing OpenRhiza with a Linux distribution.
- A named browser, firewall, memory inspector, Wi-Fi driver or compiler demo does
  not make that capability implemented. Raw kernel-memory patching is rejected.
- Previous completion percentages, frame counters and ARM proof milestones do
  not transfer to the new implementation.

## Traceability maintenance

Every new module, architecture decision and test references these requirement IDs.
Add details without silently broadening release claims. Preserve deferred goals,
but publish only the platform/features whose gates pass. When requirements conflict,
record an explicit decision in `DIRECTION.md` before adding implementation code.
