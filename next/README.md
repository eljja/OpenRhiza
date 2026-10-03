# OpenRhiza New Development Baseline

OpenRhiza is restarting from a new implementation, not migrating the old kernel.
The earlier implementation is preserved on `codex/legacy-snapshot-20261003`,
separate from this development branch. See [archive notes](../ARCHIVE.md).
This directory currently contains the new product/architecture decisions,
requirements, evidence review, development sequence, and an inventory tool.
It does not yet contain a bootable kernel or a completed operating system.

Read in this order:

1. [Product direction](docs/DIRECTION.md)
2. [Requirements and source traceability](docs/REQUIREMENTS.md)
3. [Legacy implementation review](docs/LEGACY_REVIEW.md)
4. [New development roadmap](docs/ROADMAP.md)
5. [Machine-readable legacy inventory](docs/legacy-inventory.json)

Implementation-readiness review:

- [Entry and acceptance gates](docs/IMPLEMENTATION_GATES.md)
- [Astra initial review](docs/ASTRA_PLAN_REVIEW.md)
- [Finding disposition and re-review](docs/ASTRA_REVIEW_DISPOSITION.md)

The new implementation may learn from old device experiments, failure cases and
user journeys. It must not import old kernel/service source, path dependencies,
patched bootloader, secret injection, Wasm binaries, images, keys, fixed-slot ABI,
or old success counters as its runtime or its acceptance evidence.
Independently selected, pinned and audited third-party source/specifications are
allowed; another operating system is not an OpenRhiza runtime dependency.

Current checkpoint: R0 with Astra plan-review amendments incorporated; follow-up
review is recorded in the disposition document. Stage 1.1 selections and entry
gate K remain outstanding. New kernel bring-up, toolchain/firmware qualification,
boot-contract freeze, process isolation and module admission have not passed.
The next task is Stage 1.1 in `docs/ROADMAP.md`. Use the explicitly requested
`gpt-6-astra` model for new kernel implementation after the entry gate closes.
Implementation includes iterative build/fault-test/repair work; model choice is
not evidence that the first implementation is correct.

Check the baseline from this directory:

```powershell
pwsh -NoProfile -File tools/verify-baseline.ps1
git fetch --no-tags origin codex/legacy-snapshot-20261003
pwsh -NoProfile -File tools/verify-baseline.ps1 -LegacyRef FETCH_HEAD
```

These commands verify documents and, with -LegacyRef, the pinned archive's actual
source bytes. Fetch downloads the public archive; nothing is uploaded. The frozen
inventory is not regenerated from this branch. To make a separate inventory, use
`tools/inventory-legacy.ps1 -LegacyRoot <legacy-checkout> -OutputPath <new-file.json>`.
These tools do not build or boot the new OS, invoke an LLM, or alter legacy source.

New source, when introduced, will have its own workspace, pinned toolchain,
target configuration, initial module image, SDK, build output and QEMU harness.
Old root Cargo settings/build scripts exist only in the archive; do not import
them or accidentally inherit configuration from a parent directory.

No prior milestone is carried forward. Report new OS tests, legacy reference
tests, host tests, emulator/device tests and physical-hardware tests separately.
