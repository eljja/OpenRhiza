# Program Compatibility Goals

This document records a deferred long-term goal for OpenRhiza.
It is not the current implementation target.
It is the compatibility direction the project should preserve while the core OS, sandbox runtime, and GUI become stable.

## 1. Goal

OpenRhiza should eventually be able to use programs from multiple operating system ecosystems.

That includes, over time:

- OpenRhiza-native programs
- Linux-oriented programs
- Windows-oriented programs
- macOS-oriented programs where legally and technically possible
- other portable or emulated program formats when useful

The target is not "support one binary format inside the kernel".
The target is "make programs from many ecosystems usable through OpenRhiza capabilities".

## 2. Non-Negotiable Rule

This goal must not expand the core without limit.

OpenRhiza must continue to preserve this rule:

- leave only the minimum survival path in the core
- implement compatibility through sandboxed skills, workflows, runners, loaders, translators, and capability bridges whenever possible

That means program compatibility should be built as capability stacks, not as uncontrolled kernel growth.

## 3. Preferred Architecture

OpenRhiza should not try to solve all compatibility with one giant subsystem.

Instead, it should use object-scoped compatibility capabilities such as:

- format loader skills
- ABI bridge skills
- syscall translation skills
- userspace runtime skills
- display/input bridge skills
- file and storage bridge skills
- network bridge skills
- packaging and dependency skills

Each compatibility layer should behave like an isolated object with:

- explicit target scope
- explicit request surface
- explicit resource boundary
- validation path
- rollback path

One broken compatibility layer must not silently break unrelated layers.

## 4. Execution Model

Programs should not "just run next to the OS with raw CPU access".

The intended model is:

1. OpenRhiza identifies a requested program or program type.
2. OpenRhiza selects or creates the appropriate compatibility skill stack.
3. A loader or runner skill prepares memory, ABI bindings, IO bridges, and runtime surfaces.
4. Program execution uses the real CPU when appropriate, but under OpenRhiza-controlled execution boundaries.
5. Output, files, input, and side effects are mediated through OpenRhiza runtime interfaces.
6. Validation, scoring, and rollback remain available.

In short:

- CPU execution may be native
- environment control must remain OpenRhiza-controlled

## 5. Compatibility Strategy

OpenRhiza should pursue compatibility in stages.

### Stage A: OpenRhiza-native runtimes

- OpenRhiza-defined program ABI
- sandbox-first tools
- inspectable small applications

### Stage B: Minimal foreign runtime support

- limited ELF or PE loader skills
- minimal stdout/stderr process execution
- simple static binaries
- narrow syscall or ABI bridge support

### Stage C: Broader ecosystem bridges

- Linux compatibility stacks
- Windows compatibility stacks
- portable runtime bundles
- language-specific runners

### Stage D: Full capability orchestration

- the OS detects what a program needs
- the OS queries OpenRhiza.com for existing compatibility layers
- the OS downloads or generates only what is missing
- the OS validates compatibility layers before promotion

## 6. Registry Direction

OpenRhiza.com should eventually hold compatibility capabilities such as:

- program runners
- binary format loaders
- ABI bridge skills
- runtime packs
- dependency bundles
- evaluation results for compatibility quality
- notes about stability, performance, and security

The registry should treat these the same way it treats drivers, GUI skills, and workflows:

- reusable
- inspectable
- versioned
- evaluated
- object-scoped

## 7. Current Status

This goal is intentionally deferred.

The current priority is still:

- stable recovery console
- stable sandbox runtime
- stable display handoff
- stable GUI input and rendering
- stable skill and workflow execution

OpenRhiza should not chase broad program compatibility before those foundations are dependable.

## 8. Working Principle For Future Sessions

When future Codex, ChatGPT, Gemini, or OpenRhiza-internal AI sessions work on program compatibility, they should preserve these constraints:

- do not bloat the core to gain compatibility
- prefer compatibility skills over core expansion
- keep compatibility layers isolated by object boundary
- prefer staged validation and rollback
- prefer reuse from the registry before generating new compatibility logic
- treat "all OS programs eventually" as a capability graph problem, not a monolithic kernel feature
