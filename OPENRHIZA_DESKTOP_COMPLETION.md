# OpenRhiza Desktop Completion Status

This file tracks the staged work to turn OpenRhiza into a command-first,
AI-native desktop OS while preserving the minimal recovery core.

## Current Stage

- Active milestone: 6. Public readiness verification
- Status: complete
- Last verified: milestone 6 QEMU smoke passed with
  `logs/x86-public-readiness-smoke.log`

## Milestones

1. Boot UX
   - Goal: boot into a command-first recovery shell, with explicit `/gui`,
     `/desktop`, `/recovery`, and `/desktop-status` commands.
   - Status: complete.
   - Verification:
     - `cargo build`: passed.
     - `cargo bootimage`: passed.
     - QEMU CLI smoke: passed with `/status`, `/desktop-status`, `/gui`,
       and `/recovery`.

2. Desktop compositor skill
   - Goal: promote the multi-window compositor seed to an official desktop
     skill and make `/desktop` launch it.
   - Status: complete.
   - Verification:
     - `cargo build`: passed.
     - `build_sandbox_skills.ps1`: passed; `SKDSK.WAS` is 2742 bytes and
       `SK024.WAS` is seeded.
     - QEMU CLI smoke: passed with `/desktop`, `/gui-scene`, and `/recovery`.
     - Scene evidence: desktop window handle range `500..799`, background
       handle `500`, and window labels `Command + LLM` and `System Monitor`.

3. Window protocol
   - Goal: define stable window create/update/move/resize/focus/close
     protocol and host ABI.
   - Status: complete.
   - Verification:
     - `cargo build`: passed.
     - `build_sandbox_skills.ps1`: passed; `SKDSK.WAS` is 3472 bytes and
       `SK024.WAS` is seeded.
     - `cargo bootimage`: passed.
     - QEMU protocol smoke: passed with `/desktop`,
       `/desktop-window-selftest`, `/gui-scene`, and `/recovery`.
     - QEMU close/restore smoke: passed with `/desktop-window-close`,
       `/desktop-window-restore`, `/gui-scene`, and `/recovery`.
     - Scene evidence: `System Monitor` moved to `bounds=(980,220 ...)`
       during the move/focus selftest.

4. Default apps
   - Goal: provide terminal/LLM, system monitor, skill store, and VFS explorer
     as first desktop apps.
   - Status: complete.
   - Verification:
     - `cargo build`: passed.
     - `build_sandbox_skills.ps1`: passed; `SKDSK.WAS` is 4170 bytes and
       `SK024.WAS` is seeded.
     - `cargo bootimage`: passed.
     - QEMU default app smoke: passed with `/desktop`,
       `/desktop-apps-selftest`, `/gui-scene`, and `/recovery`.
     - Scene evidence: `Terminal + LLM`, `System Monitor`, `Skill Store`,
       and `VFS Explorer` all render as desktop windows.

5. LLM-native desktop actions
   - Goal: route natural-language desktop actions through bounded,
     object-scoped machine actions.
   - Status: complete.
   - Verification:
     - `cargo build`: passed.
     - `build_sandbox_skills.ps1`: passed; `SKDSK.WAS` is 5152 bytes and
       `SK024.WAS` is seeded.
     - `cargo bootimage`: passed.
     - QEMU action smoke: passed with `/desktop`,
       `/desktop-action focus vfs explorer`, `/gui-scene`, and `/recovery`.
     - QEMU rejection smoke: passed with `/desktop-action delete every file`
       rejected as unbounded.
     - Scene evidence: VFS Explorer window handles `570` and `571` become
       `interaction=Focused`.

6. Public readiness verification
   - Goal: complete build, QEMU, recovery, documentation, and release gates.
   - Status: complete.
   - Verification:
     - `cargo build`: passed.
     - `build_sandbox_skills.ps1`: passed; `SKDSK.WAS` is 5152 bytes and
       `SK024.WAS` is seeded.
     - `cargo bootimage`: passed; `bootimage-OpenRhiza.bin` is 7040000 bytes.
     - QEMU public readiness smoke: passed with `/status`,
       `/desktop-status`, `/desktop`, `/desktop-apps-selftest`,
       `/desktop-action focus vfs explorer`, `/gui-scene`,
       `/desktop-action delete every file`, and `/recovery`.
     - Scene evidence: all four default apps render, VFS Explorer is focused
       by the bounded action route, unbounded desktop action is rejected, and
       recovery remains available.

## Verification Rules

- Each milestone must pass build checks before QEMU.
- Each milestone must pass QEMU smoke before the next milestone starts.
- Recovery shell and command input must remain available after GUI or desktop
  experiments.
