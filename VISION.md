# The Vision of OpenRhiza

The long-term goal of OpenRhiza is not just to boot an OS with an LLM attached.
It is to build an AI-native operating system where the machine can expand itself safely, explain what it is doing, and evolve most of its behavior through isolated sandbox capabilities instead of uncontrolled kernel growth.

Architecture authority: `OPENRHIZA_MODULE_ARCHITECTURE.md` and
`OPENRHIZA_REBUILD_PLAN.md` define the current minimal-core and isolation boundary.
The OS/Nexus/forge ecosystem review and implementation gaps are recorded in
`OPENRHIZA_COM_PLAN.md` (2026-10-03). A functioning catalog or sandbox demonstration
does not establish a safe self-improving operating system.

## 1. The Core Principle

OpenRhiza should always preserve one rule:

- leave only the minimum and mandatory survival path in the core
- implement everything else through sandboxed skills, workflows, drivers, programs, and object-capabilities whenever possible

The core is not where new product features should accumulate.
The core should exist to:

- boot the machine
- enforce address spaces, preemption, capability/IPC and bounded device grants
- reserve recovery resources and reinstantiate a sealed signed recovery set
- provide minimal early diagnostic output and recovery handoff

Networking, storage, the Wasm runtime, recovery UI, validation orchestration,
rollback policy and LLM planning belong to isolated modules. Existing native
bootstrap paths are temporary, not an exception to the module invariant.

## 2. The Evolution Path

OpenRhiza grows through staged capability evolution:

1. **Recovery survival path**
   - minimal console
   - minimal input
   - signed offline bootstrap/recovery modules
2. **Sandbox capability bring-up**
   - fetch existing drivers, skills, and workflows from OpenRhiza.com
   - generate missing capabilities through LLMs when needed
   - independently build/test them, then admit them into isolated user-mode Wasm
     or native workers with explicit capabilities and resource limits
3. **Stable runtime handoff**
   - activate validated capabilities live
   - persist only after success
   - roll back immediately on failure
4. **Self-hosted improvement**
   - let the OS inspect and improve its own UI, workflows, and capability graph from inside the machine

## 3. Object-Oriented Capability Model

OpenRhiza should treat more than GUI elements as objects.

The same object rule should apply to:

- GUI items
- drivers
- skills
- workflows
- runtime services
- programs

Each object should have:

- stable identity
- explicit bounds or operating scope
- declared request surface
- isolated lifecycle
- isolated rollback path

One broken object should not silently break unrelated ones.

## 4. Generative Space

OpenRhiza should not behave like a traditional OS plus package manager.

If the user asks for a capability, the OS should:

1. inspect local state
2. query the capability registry
3. reuse known-good work first
4. generate only what is still missing
5. validate before promotion
6. report back what happened

That applies to drivers, software, workflows, and eventually the GUI itself.

## 5. The Registry And Shared Memory

OpenRhiza.com is not an app store.
It is shared operational memory for OpenRhiza nodes.

It should hold:

- capabilities
- artifacts
- evaluations
- comments
- votes
- policies
- workflows
- models
- node profiles

The module forge and registry are separate trust domains. Generated source,
community reports and model votes are untrusted inputs. Tests, publication policy,
signing and local admission are independently enforced. Field evidence must name
the exact artifact, platform and test environment; popularity does not certify
correctness. Private semantic files, prompts and voice data remain local unless
the user authorizes a bounded upload. Offline boot and installed modules must
remain available if either the registry or a model fails.

The OS should use this registry as part of its reasoning loop, not just as a download bucket.

## 6. Self-Hosted GUI And Self-Hosted Development

The target end state is that OpenRhiza can improve its own interface from inside its own console and GUI.

That means:

- the GUI must be modeled as a scene of isolated objects
- the scene must be inspectable
- mutations must be object-scoped
- sandbox skills and LLM actions must be able to request those mutations safely

External development support is acceptable during bootstrap.
It is not the final state.
The final state is that OpenRhiza can design and refine its own runtime capabilities from within OpenRhiza itself.
