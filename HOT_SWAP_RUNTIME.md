# Hot-Swap Runtime Architecture

This document defines the long-term runtime direction for OpenRhiza.

The goal is:

- users should express intent with prompts
- the OS should react without asking users to manage drivers, filesystems, or system internals
- most changes should apply without reboot

## Core Principle

Only a very small core should require reboot-level trust.

This must remain a hard architectural rule, not a temporary preference:

- the core is for survival and isolation only
- heavy device-specific logic must not creep back into the core
- the first answer to new hardware support should be a sandbox component, not a new built-in kernel driver

Everything else should move toward:

- sandbox-first execution
- staged validation
- active binding switch
- rollback on failure

## What Should Stay in Core

The immutable or near-immutable core should stay minimal:

- boot path
- allocator and memory basics
- interrupt path
- scheduler or executor
- survival display path
- survival keyboard path
- minimal network path
- minimal storage read path
- sandbox loader and isolation boundary

If a component is not part of basic system survival, it should be a candidate for hot-swap.

If a new feature can be expressed as a sandbox component with a stable host ABI, that path should be preferred over enlarging the core.

## What Should Become Hot-Swappable

OpenRhiza should eventually treat these as runtime-loadable modules:

- hardware drivers
- input parser drivers
- storage policy modules
- filesystem adapters
- registry sync logic
- network protocol adapters
- software tools
- generated programs
- high-level automation logic

## Runtime States

Each loadable component should move through explicit states:

1. `discovered`
2. `staged`
3. `testing`
4. `active`
5. `rollback`
6. `deprecated`

This is more important than whether the component is local, remote, generated, or downloaded.

## Sandbox-First Policy

The sandbox should not only be a laboratory for experiments.

It should become the default entry path for:

- new drivers
- replacement drivers
- new filesystem behavior
- new storage logic
- generated services

Promotion to active use should happen only after validation.

## Active Binding Model

OpenRhiza should separate:

- the currently active runtime binding
- the persisted preferred binding for later boots

This means:

- runtime activation can happen immediately
- persistent preference can be updated only after success
- reboot is optional, not the main way to adopt a change
- local runtime bindings should be inspectable and adjustable without reboot

In practice, OpenRhiza should keep:

- a live runtime binding map used by the running system now
- a persisted preferred binding map used for later boots

These should not be treated as the same thing.

## Driver Hot-Swap Policy

When a driver candidate becomes available:

1. identify the hardware match key
2. stage the candidate
3. run sandbox smoke tests
4. check for keyboard, display, network, and storage regressions
5. if successful, switch the active driver binding
6. if the system remains stable, persist the preferred binding for later boots
7. upload evaluation and comments to OpenRhiza.com

OpenRhiza should increasingly represent drivers as generic runtime components with:

- a match key
- a component key
- a sandbox lifecycle state
- a live binding state
- a persisted preferred binding state

That is the preferred long-term model for non-core hardware support.

The running system should also support:

- inspecting current live bindings
- manually activating a different candidate for a match key
- immediate rollback to the previous live binding if the new one is unsafe

Input drivers should follow the same policy.

For keyboard and mouse support, the preferred sequence is:

1. raw HID handoff from core
2. sandbox parser activation in mirrored mode
3. sandbox-preferred runtime ownership
4. rollback to bootstrap fallback if regression is detected

Current operator commands:

- `/sandbox-mouse-load`
- `/sandbox-keyboard-load`
- `/input-routing-status`
- `/input-activate <keyboard|mouse>`
- `/input-rollback <keyboard|mouse>`

The current implementation also supports persisted `input:keyboard` and `input:mouse` bindings, which are restored automatically on the next boot when the local driver artifact is available.

## Filesystem and Storage Hot-Swap Policy

Filesystem behavior should be layered, not monolithic.

Prefer separate modules for:

- block driver
- partition parser
- filesystem adapter
- cache policy
- persistence policy

That allows:

- new filesystem support without reboot
- safer storage evolution
- more targeted rollback when storage changes fail

Storage writes remain higher risk than many other modules, so write-capable storage paths must use stricter validation than read-only components.

## Reboot Policy

Reboot should be a last resort.

OpenRhiza should prefer:

- hot-load
- staged activation
- live rollback

Use reboot only when:

- the core path changes
- the allocator or low-level memory model changes
- the interrupt model changes
- the sandbox boundary itself changes

## User-Facing Rule

The user should not need to think about:

- partitions
- filesystem layout
- driver installation order
- cache invalidation
- reboot timing

The user should only express intent.

OpenRhiza should decide:

- what to reuse
- what to generate
- what to stage
- what to activate
- what to persist
- what to roll back

## Practical Direction For This Repository

The repository should move toward:

- local active binding maps
- local persisted preferred binding maps
- staged generated artifacts
- sandbox-first validation for non-core changes
- reboot-free activation whenever the changed component is outside the core

That is the correct architectural direction for an AI-native OS rather than a traditional static OS.
