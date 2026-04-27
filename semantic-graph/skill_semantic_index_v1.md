# skill_semantic_index_v1

## Role

Build a sidecar semantic graph for a managed filesystem root so OpenRhiza can query structured file knowledge before loading raw file contents into LLM context.

## Current backend

- host-side scanner
- node/edge artifact generation
- chunked text indexing
- basic path-reference extraction

## Output

- `manifest.json`
- `nodes.ndjson`
- `edges.ndjson`
- `keywords.json`

## Initial node kinds

- `filesystem_root`
- `directory`
- `file`
- `text_chunk`

## Initial edge kinds

- `contains`
- `chunk_of`
- `references_path`
- `same_name_family`

## Why it should not start as a new filesystem

The semantic graph is currently a semantic overlay, not a raw storage substrate.
Keeping it as a sidecar capability preserves interoperability and avoids unnecessary core/storage complexity.
