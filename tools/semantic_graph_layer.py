from __future__ import annotations

import argparse
import hashlib
import json
import mimetypes
import re
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable


TEXT_EXTENSIONS = {
    ".md",
    ".txt",
    ".json",
    ".toml",
    ".yaml",
    ".yml",
    ".rs",
    ".py",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".c",
    ".h",
    ".cpp",
    ".hpp",
    ".ini",
    ".log",
}
SKIP_DIRS = {
    ".git",
    ".fslab",
    "target",
    "target-alt",
    "logs",
    "__pycache__",
}
PATH_REF_RE = re.compile(r"([A-Za-z0-9_.\-/]+(?:\.[A-Za-z0-9_#-]+)+)")
WORD_RE = re.compile(r"[A-Za-z0-9_]{3,}")


@dataclass
class Node:
    id: str
    kind: str
    label: str
    path: str | None = None
    hash: str | None = None
    size: int | None = None
    mime: str | None = None
    chunk_index: int | None = None
    text_preview: str | None = None


@dataclass
class Edge:
    src: str
    dst: str
    kind: str


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def stable_id(kind: str, path: str) -> str:
    digest = hashlib.sha256(f"{kind}:{path}".encode("utf-8")).hexdigest()[:16]
    return f"{kind}_{digest}"


def relpath(root: Path, path: Path) -> str:
    return path.relative_to(root).as_posix() if path != root else "."


def chunk_text(text: str, max_chars: int = 1200) -> list[str]:
    chunks: list[str] = []
    current: list[str] = []
    length = 0
    for line in text.splitlines(keepends=True):
        if length + len(line) > max_chars and current:
            chunks.append("".join(current))
            current = []
            length = 0
        current.append(line)
        length += len(line)
    if current:
        chunks.append("".join(current))
    return chunks


def extract_keywords(text: str) -> list[str]:
    counts: dict[str, int] = {}
    for word in WORD_RE.findall(text.lower()):
        counts[word] = counts.get(word, 0) + 1
    ranked = sorted(counts.items(), key=lambda item: (-item[1], item[0]))
    return [word for word, _ in ranked[:32]]


def detect_text(path: Path) -> bool:
    if path.suffix.lower() in TEXT_EXTENSIONS:
        return True
    guessed, _ = mimetypes.guess_type(path.name)
    return guessed is not None and guessed.startswith("text/")


def iter_paths(root: Path) -> Iterable[Path]:
    yield root
    for path in root.rglob("*"):
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        yield path


def build_graph(root: Path) -> tuple[list[Node], list[Edge], dict[str, list[str]]]:
    nodes: list[Node] = []
    edges: list[Edge] = []
    keywords: dict[str, list[str]] = {}
    path_to_node: dict[str, str] = {}

    root_rel = relpath(root, root)
    root_id = stable_id("filesystem_root", root_rel)
    nodes.append(Node(id=root_id, kind="filesystem_root", label=root.name or "root", path=root_rel))
    path_to_node[root_rel] = root_id

    all_paths = list(iter_paths(root))
    for path in all_paths:
        if path == root:
            continue
        rel = relpath(root, path)
        parent_rel = relpath(root, path.parent) if path.parent else "."
        if path.is_dir():
            node_id = stable_id("directory", rel)
            nodes.append(Node(id=node_id, kind="directory", label=path.name, path=rel))
            path_to_node[rel] = node_id
            if parent_rel in path_to_node:
                edges.append(Edge(src=path_to_node[parent_rel], dst=node_id, kind="contains"))
            continue

        data = path.read_bytes()
        guessed_mime, _ = mimetypes.guess_type(path.name)
        node_id = stable_id("file", rel)
        file_hash = sha256_bytes(data)
        nodes.append(
            Node(
                id=node_id,
                kind="file",
                label=path.name,
                path=rel,
                hash=file_hash,
                size=len(data),
                mime=guessed_mime,
            )
        )
        path_to_node[rel] = node_id
        if parent_rel in path_to_node:
            edges.append(Edge(src=path_to_node[parent_rel], dst=node_id, kind="contains"))

        if not detect_text(path):
            continue
        text = data.decode("utf-8", errors="replace")
        keywords[rel] = extract_keywords(text)
        chunks = chunk_text(text)
        for index, chunk in enumerate(chunks):
            chunk_id = stable_id("text_chunk", f"{rel}:{index}")
            nodes.append(
                Node(
                    id=chunk_id,
                    kind="text_chunk",
                    label=f"{path.name}#{index}",
                    path=rel,
                    chunk_index=index,
                    text_preview=chunk[:220],
                )
            )
            edges.append(Edge(src=node_id, dst=chunk_id, kind="chunk_of"))

    # second pass for path references and same-name families
    by_name: dict[str, list[str]] = {}
    for node in nodes:
        if node.kind == "file" and node.path:
            by_name.setdefault(Path(node.path).name.lower(), []).append(node.id)

    file_nodes = [node for node in nodes if node.kind == "file" and node.path]
    file_map = {node.path: node.id for node in file_nodes if node.path}
    for node in file_nodes:
        if not node.path:
            continue
        path = root / node.path
        if not detect_text(path):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for match in PATH_REF_RE.findall(text):
            ref = match.strip().strip("`\"'()[]{}<>")
            if not ref:
                continue
            normalized = Path(ref).as_posix()
            if normalized in file_map:
                edges.append(Edge(src=node.id, dst=file_map[normalized], kind="references_path"))
            else:
                basename = Path(normalized).name.lower()
                for candidate in by_name.get(basename, []):
                    if candidate != node.id:
                        edges.append(Edge(src=node.id, dst=candidate, kind="same_name_family"))

    return nodes, edges, keywords


def write_ndjson(path: Path, rows: Iterable[dict]) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        for row in rows:
            handle.write(json.dumps(row, ensure_ascii=False) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser(description="Build an OpenRhiza sidecar semantic graph for a filesystem root")
    parser.add_argument("--root", type=Path, required=True, help="filesystem root to index")
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="output graph directory (defaults to <root>/.openrhiza/semantic-graph)",
    )
    args = parser.parse_args()

    root = args.root.resolve()
    output = args.output.resolve() if args.output else root / ".openrhiza" / "semantic-graph"
    output.mkdir(parents=True, exist_ok=True)

    nodes, edges, keywords = build_graph(root)
    manifest = {
        "root": str(root),
        "output": str(output),
        "node_count": len(nodes),
        "edge_count": len(edges),
        "kinds": {
            "filesystem_root": sum(1 for node in nodes if node.kind == "filesystem_root"),
            "directory": sum(1 for node in nodes if node.kind == "directory"),
            "file": sum(1 for node in nodes if node.kind == "file"),
            "text_chunk": sum(1 for node in nodes if node.kind == "text_chunk"),
        },
    }

    (output / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False), encoding="utf-8")
    write_ndjson(output / "nodes.ndjson", (asdict(node) for node in nodes))
    write_ndjson(output / "edges.ndjson", (asdict(edge) for edge in edges))
    (output / "keywords.json").write_text(json.dumps(keywords, indent=2, ensure_ascii=False), encoding="utf-8")

    print(f"Indexed {root}")
    print(f"nodes={len(nodes)} edges={len(edges)} output={output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
