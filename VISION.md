# The Vision of OpenRhiza

The long-term goal of OpenRhiza is not just to boot an OS with an LLM attached.
It is to build an AI-native operating system where the machine can expand itself safely, explain what it is doing, and evolve most of its behavior through isolated sandbox capabilities instead of uncontrolled kernel growth.

## 1. The Core Principle

OpenRhiza should always preserve one rule:

- leave only the minimum and mandatory survival path in the core
- implement everything else through sandboxed skills, workflows, drivers, programs, and object-capabilities whenever possible

The core is not where new product features should accumulate.
The core should exist to:

- boot the machine
- preserve recovery input and recovery display
- provide minimal networking and storage bootstrap
- run the Wasm sandbox
- enforce validation, rollback, and capability boundaries

## 2. The Evolution Path

OpenRhiza grows through staged capability evolution:

1. **Recovery survival path**
   - minimal console
   - minimal input
   - minimal network/bootstrap storage
2. **Sandbox capability bring-up**
   - fetch existing drivers, skills, and workflows from OpenRhiza.com
   - generate missing capabilities through LLMs when needed
   - validate them in Wasm first
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
