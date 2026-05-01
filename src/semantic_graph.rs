use alloc::format;
use alloc::string::String;

pub fn status_block() -> String {
    let storage = crate::storage_host::harness_descriptor();
    let registry_context = crate::api_v1::current_registry_context_block().is_some();
    let wasm_modules = crate::os_core_seed::wasm_health_snapshot().len();

    let mut out = String::from("[Semantic Graph] sidecar graph layer\n");
    out.push_str("- core_policy: skill-owned, sidecar-only, rebuildable\n");
    out.push_str("- graph_root: .openrhiza/semantic-graph/\n");
    out.push_str("- artifacts: manifest.json, nodes.ndjson, edges.ndjson, keywords.json\n");
    out.push_str(
        format!(
            "- storage_harness: {}\n",
            if storage.is_some() { "available" } else { "missing" }
        )
        .as_str(),
    );

    if let Some(descriptor) = storage {
        out.push_str(
            format!(
                "- image: fs={} writable={} blocks={} scratch_start={} scratch_blocks={}\n",
                descriptor.fs_hint.as_str(),
                descriptor.writable as u8,
                descriptor.filesystem_block_count,
                descriptor.scratch_start_lba,
                descriptor.scratch_block_count
            )
            .as_str(),
        );
    }

    out.push_str(
        format!(
            "- registry_context: {}\n",
            if registry_context { "available" } else { "not-yet-fetched" }
        )
        .as_str(),
    );
    out.push_str(format!("- loaded_wasm_modules: {}\n", wasm_modules).as_str());
    out.push_str("- next_skill: skill_semantic_graph_index_v1\n");
    out.push_str("- safety: graph writes must stay inside the graph_root object namespace");
    out
}
