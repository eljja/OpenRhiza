# OpenRhiza Autonomy Mode

This document defines how OpenRhiza should provide autonomous help without becoming uncontrolled.

## 1. Goal

OpenRhiza should not wait passively for every tiny instruction.

It should continuously:

- infer the user's intent
- infer the user's medium-term goal
- detect likely blockers
- prepare useful options
- gather bounded evidence
- propose practical next steps

However:

- it must not become dictatorial
- it must not silently take irreversible action
- it must not replace the user's agency

The target behavior is:

- proactive
- evidence-seeking
- bounded
- reversible
- user-aligned

## 2. Core Principle

Autonomy is not permission for unilateral execution.

OpenRhiza may:

- observe
- infer
- inspect
- compare options
- run bounded non-destructive checks
- prepare drafts
- prepare artifacts
- show likely outcomes

OpenRhiza must still ask before:

- destructive storage changes
- persistent system-wide activation
- irreversible promotion
- network publication of newly created artifacts
- replacing active user-visible behavior in a disruptive way

## 3. On or Off at First Boot

Autonomy should be a first-boot option.

Recommended first-boot modes:

1. `Off`
   - OpenRhiza acts only on direct user prompts.
   - No proactive suggestions.
   - No autonomous background planning.

2. `Assist`
   - OpenRhiza may watch context and suggest next steps.
   - It may gather bounded evidence.
   - It may prepare drafts or candidate actions.
   - It must ask before meaningful execution.

3. `Council`
   - OpenRhiza runs the three-agent autonomy model described below.
   - It may prepare and compare multiple plans.
   - It may carry bounded safe checks further before asking.
   - It still asks before promotion, persistence, destructive operations, or public upload.

The user should be able to switch this later, but first boot should explicitly set the starting policy.

Current bootstrap implementation:

- default mode: `Off`
- default interval: `10` minutes
- stale council timeout: about `120` seconds
- current council backend: Gemini
- council roles: `practical`, `analytical`, `bold`
- council responses are handled as autonomy-origin prompts and must not execute machine-action JSON
- recent GUI context extraction is UTF-8 safe
- stale council cycles are cleared automatically with hold votes so the OS does not get stuck
- user commands:
  - `/autonomy-status`
  - `/autonomy-mode <off|assist|council>`
  - `/autonomy-interval <minutes>`
  - `/autonomy-run-now`

The AI itself must not change the interval or autonomy mode.

## 4. Three-AI Council Model

Autonomy should not come from one unchecked planner.

OpenRhiza should use three independent AI agents and choose proposals by majority vote.

Each agent should:

- receive the same current system state
- receive the same current user prompt history
- reason independently
- produce:
  - intent estimate
  - goal estimate
  - risk estimate
  - recommended next action
  - confidence

Recommended personality split:

1. `Practical`
   - optimizes for reliability and immediate usefulness
   - prefers stable existing solutions
   - dislikes risky experiments

2. `Analytical`
   - optimizes for technical correctness and long-term structure
   - prefers explicit evidence and validation
   - catches hidden failure modes

3. `Bold`
   - optimizes for capability growth and ambitious improvement
   - looks for better-than-current solutions
   - proposes stronger upgrades when the risks are justified

Alternative split is also acceptable:

- emotional / rational / ethical
- realistic / efficient / ambitious
- conservative / balanced / exploratory

The exact personality labels are less important than keeping the three decision styles meaningfully different.

## 5. Model Choice

The three agents may be:

- three different LLMs
- one primary LLM plus two alternates
- one single LLM with three strongly separated system roles

Preference order:

1. different models when available
2. same model with different system instructions when needed

If the same model is reused, role separation must be explicit and stable.

## 6. Decision Rule

Each council cycle should produce:

- `intent`
- `goal`
- `constraints`
- `candidate action`
- `safe evidence already gathered`
- `needs user confirmation? yes/no`

Then OpenRhiza should:

1. compare the three proposals
2. merge overlapping agreement
3. choose the majority-supported action
4. present the user with:
   - the inferred goal
   - the recommended next step
   - evidence already gathered
   - what remains uncertain
   - whether approval is needed

If no majority exists:

- do not act
- present the disagreement briefly
- ask the user to choose direction

## 7. Bounded Autonomous Evidence Gathering

Autonomy is most useful when it can do some safe work before asking.

Allowed bounded autonomous work:

- inspect current hardware
- inspect current drivers, skills, workflows, policies
- inspect current GUI scene and object state
- compare local cache vs registry
- perform safe read-only filesystem inspection
- prepare candidate driver or skill artifacts
- compile candidate artifacts in sandbox
- run bounded sandbox smoke tests
- draft UI mutations without promoting them
- prepare a summary with likely results

This is the preferred behavior:

- do the safe groundwork
- show concrete prepared options
- ask only for the final higher-risk adoption decision

## 8. Explicit Safety Boundaries

Even in `Council` mode, OpenRhiza must not autonomously:

- wipe or repartition storage
- overwrite trusted persistent bindings
- promote unvalidated drivers
- replace active recovery paths
- upload private local content publicly
- publish to OpenRhiza.com without policy permission
- permanently switch the active system behavior in a way the user cannot easily reverse

## 9. Relation To Core and Sandbox

The autonomy system itself should follow the OpenRhiza philosophy.

That means:

- core should only keep minimal autonomy state and gating
- planners, council logic, proposal ranking, and evidence gathering should be implemented as skills or workflows where possible
- execution should be object-scoped
- each agent should be isolated as a capability object

Do not hardcode large decision logic into the core.

Current implementation note:

- the core currently contains a minimal coordinator and persistence gate
- queued council roles are tracked only after successful Gemini prompt queueing
- the next hardening item is stale-cycle timeout recovery for network/model failure cases

## 10. Suggested Object Model

Recommended autonomy objects:

- `autonomy.mode`
- `autonomy.council`
- `autonomy.agent.practical`
- `autonomy.agent.analytical`
- `autonomy.agent.bold`
- `autonomy.proposal`
- `autonomy.evidence_bundle`
- `autonomy.vote_result`
- `autonomy.approval_gate`

Each object should have:

- identity
- lifecycle
- logs
- confidence
- rollback or discard path

## 11. Initial Execution Flow

Suggested early implementation flow:

1. User enters a prompt.
2. OpenRhiza checks whether autonomy mode is `Off`, `Assist`, or `Council`.
3. In `Off`:
   - direct execution only
4. In `Assist`:
   - one planner proposes next steps
   - safe evidence may be gathered
   - confirmation is requested before meaningful action
5. In `Council`:
   - three agents independently produce proposals
   - majority result is assembled
   - bounded evidence is shown
   - user confirms adoption when needed

## 12. UX Recommendation

OpenRhiza should present autonomy results like this:

- `Inferred goal: ...`
- `Likely blocker: ...`
- `Prepared option: ...`
- `Evidence gathered: ...`
- `Recommended next step: ...`
- `Needs approval: yes/no`

The system should feel helpful, not invasive.

## 13. Long-Term Direction

Later, OpenRhiza should be able to:

- autonomously maintain driver quality
- proactively suggest filesystem repair or indexing refresh
- prepare GUI improvements
- prepare workflow optimizations
- summarize important changes in the local machine state

But all of that should still remain:

- explainable
- reviewable
- bounded
- reversible

## 14. Final Rule

Autonomy should increase usefulness, not reduce trust.

If a proposed autonomy behavior would make OpenRhiza less predictable, less reversible, or more fragile, it is the wrong design.
