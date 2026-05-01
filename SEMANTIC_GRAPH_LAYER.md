# Semantic Graph Layer

This document defines the first OpenRhiza design for a filesystem-aware semantic graph layer.

## 1. Why It Exists

OpenRhiza is an LLM-native operating system.
That means the OS should not treat files as opaque blobs unless absolutely necessary.

If a capability, driver, workflow, program, or user document already exists inside a managed filesystem, the OS should be able to:

- discover it
- identify what it is
- connect it to related objects
- expose it to the LLM as structured context

This is a key long-term capability for an AI OS.

## 2. Core Decision

The semantic graph layer should **not** start as a separate filesystem.

The preferred initial model is:

- keep existing filesystems for raw storage
- add a **sidecar semantic graph layer** on top
- store semantic graph artifacts as normal files inside the managed filesystem or alongside mounted images

Why:

- less risk
- easier validation
- preserves compatibility with FAT32, exFAT, NTFS, and ext family storage
- keeps the graph layer replaceable
- avoids coupling the AI index to one physical storage format

## 3. When A Separate Filesystem Might Be Useful

A separate graph-oriented filesystem or partition may become useful later if:

- graph scale becomes very large
- frequent semantic mutation causes excessive rewrite cost
- graph queries need snapshot-heavy or append-only behavior
- the graph becomes a durable system database rather than a cacheable semantic surface

That is not the current priority.

## 4. Initial Architecture

Use an overlay-style sidecar graph:

- raw files remain in the original filesystem
- semantic artifacts are written into a reserved graph directory
- the graph references raw files by stable path and content hash

Recommended reserved directory:

- `.openrhiza/semantic-graph/`

Recommended base artifacts:

- `manifest.json`
- `nodes.ndjson`
- `edges.ndjson`
- `keywords.json`

## 5. Object Model

The graph should treat filesystem content as objects.

Initial node kinds:

- `filesystem_root`
- `directory`
- `file`
- `text_chunk`

Initial edge kinds:

- `contains`
- `chunk_of`
- `references_path`
- `same_name_family`

Later node kinds may include:

- `driver`
- `skill`
- `workflow`
- `program`
- `font`
- `prompt`
- `registry_artifact`
- `evaluation`

## 6. LLM Usage Model

The LLM should not be forced to scan raw storage every time.

Instead:

1. the graph index is refreshed
2. OpenRhiza asks the graph for relevant objects
3. the graph returns:
   - candidate files
   - summaries
   - chunk references
   - relationship edges
4. only then does the OS load the raw content needed for the current task

This keeps LLM context small while preserving access to the whole managed filesystem.

## 7. Update Strategy

The graph should be refreshable in stages:

- full scan
- subtree scan
- single file refresh
- on-write refresh
- scheduled maintenance refresh

The graph should be treated as rebuildable.
The source of truth remains the underlying filesystem content.

## 8. Initial Deliverable

The first implementation should have two layers:

1. a host-side validation tool that:
   - scans a filesystem root
   - builds node and edge artifacts
   - chunks text files
   - extracts simple path references
   - emits graph artifacts into a sidecar directory
2. an OpenRhiza-side sandbox skill that:
   - consumes a bounded directory or image view through the storage host ABI
   - emits the same `manifest.json`, `nodes.ndjson`, and `edges.ndjson` format
   - updates only its own graph object files
   - returns compact object references to the LLM instead of dumping raw storage content

The host tool is useful for speed and comparison, but the OS requirement is that the same capability can run as a bounded skill inside OpenRhiza.
The graph builder must never become a hidden core service.

Current OS-side bootstrap:

- `/semantic-status` reports whether the storage host ABI is visible from OpenRhiza.
- It reports the expected graph root, artifact names, registry context availability, and loaded Wasm module count.
- The current implementation is deliberately introspection-only in core.
- The actual graph builder target remains `skill_semantic_graph_index_v1`.
- Graph writes must be object-scoped under `.openrhiza/semantic-graph/` so a broken indexer cannot corrupt unrelated filesystem objects.

## 9. Long-Term Direction

Eventually the semantic graph layer should become:

- queryable from inside OpenRhiza
- refreshable through skills/workflows
- usable by GUI, registry, and internal planning
- part of the capability selection and self-improvement loop

But it should still preserve the main rule:

- keep the core small
- let the graph layer behave like a bounded capability object
