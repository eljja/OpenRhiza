# OpenRhiza Public Alpha Checklist

This checklist defines the minimum practical bar before OpenRhiza should be shown as a publicly usable alpha.

It is not a feature wishlist. It is the smallest set of reliability and safety conditions needed before outside users can test the system without constantly needing internal project context.

## Release Principle

Public alpha must preserve the OpenRhiza rule:

- keep the core minimal and survival-focused
- move non-survival behavior into sandboxed capabilities
- keep recovery input, recovery display, rollback, and local inspection available
- never hide known fragility behind marketing language

## Required Before Public Alpha

| Area | Required State | Current State |
| --- | --- | --- |
| x86 boot | QEMU consistently reaches usable GUI or recovery shell. | Partial: current path works, still needs repeated clean boot runs. |
| Recovery shell | Must remain reachable after GUI or skill failure. | Partial: recovery path exists, needs regression matrix. |
| Keyboard/mouse | Input must survive GUI handoff and sandbox input experiments. | Partial: usable now; right Shift and USB numpad issues remain. |
| GUI | Must support long prompt/response sessions with retained history and no major redraw artifacts. | Partial: object GUI works; long-session scroll/history still needs hardening. |
| Gemini/OpenRhiza API | Plain prompts should reach Gemini when a build-time key is present. | Partial: works in current x86 path; needs automated smoke test. |
| Registry loop | Query/download/cache/load should work for drivers and skills. | Partial: works for seed skills and known registry paths; artifact maturity varies. |
| Sandbox runtime | Multiple named modules should not starve or corrupt each other. | Partial: bounded polling exists; per-module quotas are not complete. |
| Driver model | At least one major device path should prove sandbox-driver replacement with native fallback. | Not complete: e1000 is the priority. |
| Storage | Bootstrap cache writes should be safe; filesystem harness should validate scratch writes. | Partial: fixed-slot write floor exists; general filesystem bridge is pending. |
| Filesystems | FAT32, exFAT, NTFS, ext2/3/4 should be validated through image-backed skills or lab tools. | Partial: design and harness exist; full in-OS validation is pending. |
| Semantic graph | Managed files should be discoverable as structured context. | Not complete: design exists; skill implementation pending. |
| Autonomy | Off by default; assist/council modes must be bounded and user-controlled. | Partial: commands and council exist; workflow migration remains. |
| Voice | Optional and off by default; route chain must not break keyboard or GUI. | Partial: route-aware sandbox chain exists; real capture is pending. |
| ARM64 | First non-x86 platform should show survival boot without expanding core. | Partial pass: ARM64 serial recovery ELF builds and smoke-boots. |
| Documentation | Public docs must separate implemented, partial, and future work. | In progress. |

## Public Alpha Blockers

1. e1000 native path is still the active network device driver.
2. xHCI/HID and IME/keymap logic still contain bootstrap core debt.
3. GUI shell policy still lives too much in bootstrap rendering code.
4. Long GUI sessions need repeatable retention and scroll validation.
5. General filesystem read/write is not yet proven inside OpenRhiza.
6. Per-module Wasm quota/accounting/quarantine is not complete.
7. Public docs need a clear "what works now" vs "what is planned" split.

## Acceptable Public Alpha Scope

The first public alpha may honestly claim:

- x86_64 QEMU boot and recovery shell
- 1920x1080 bootstrap GUI
- prompt-first interaction with Gemini when configured
- OpenRhiza.com registry integration for known capability records
- sandbox skill loading and object-scoped GUI mutation path
- early voice route chain
- early ARM64 serial recovery proof

It should not yet claim:

- full hardware compatibility
- production filesystem support
- complete GUI desktop polish
- autonomous self-improvement without supervision
- Android or phone install support
- all drivers running outside core

## Recommended Alpha Verification Script

Before tagging an alpha candidate, run and record:

1. `cargo build`
2. `cargo bootimage`
3. `pwsh.exe -ExecutionPolicy Bypass -File .\build_sandbox_skills.ps1`
4. `python .\tools\platform_matrix.py --strict`
5. `pwsh.exe -ExecutionPolicy Bypass -File .\build_arm64_recovery.ps1`
6. `python .\tools\smoke_arm64_recovery.py`
7. x86 QEMU manual smoke:
   - GUI appears
   - keyboard and mouse work
   - plain Gemini prompt works
   - `/platform-status` works
   - `/voice-status` works
   - `/wasm-status` works
   - recovery shell still responds after GUI session switching

## Documentation Gate

Before public alpha, update:

- `README.md`
- `ROADMAP.md`
- `KNOWN_ISSUES.md`
- `BUILD_AND_TEST.md`
- `VALIDATION_MATRIX.md`
- release notes or `CHANGELOG.md`
