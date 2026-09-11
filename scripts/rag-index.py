#!/usr/bin/env python3
"""Index sanitized Jarvis documentation in LiteLLM + Qdrant without extra deps."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import sys
import urllib.error
import urllib.request
import uuid

DEFAULT_PATTERNS = ("README.md", "STATUS.md", "docs/**/*.md", "integrations/**/README.md")
MAX_FILE_BYTES = 512 * 1024
MAX_CHUNK_CHARS = 1800
NAMESPACE = uuid.UUID("68bca99b-71df-49b7-a88f-c46ec72fe2bb")
SECRET_PATTERN = re.compile(
    r"(?i)(?:api[_-]?key|secret|password|token)\s*[:=]\s*['\"]?[A-Za-z0-9_./+=-]{20,}"
)


def request_json(method: str, url: str, body: object | None = None, token: str | None = None) -> object:
    data = None if body is None else json.dumps(body).encode()
    headers = {"Accept": "application/json"}
    if data is not None:
        headers["Content-Type"] = "application/json"
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(url, data=data, headers=headers, method=method)
    with urllib.request.urlopen(request, timeout=60) as response:
        if response.length is not None and response.length > 8 * 1024 * 1024:
            raise ValueError("upstream response is too large")
        return json.load(response)


def chunks(text: str) -> list[tuple[str, str]]:
    heading = "Documento"
    pending: list[str] = []
    output: list[tuple[str, str]] = []
    length = 0

    def flush() -> None:
        nonlocal pending, length
        if pending:
            output.append((heading, "\n\n".join(pending)))
            pending = []
            length = 0

    for block in re.split(r"\n\s*\n", text):
        block = block.strip()
        if not block:
            continue
        match = re.match(r"^#{1,4}\s+(.+)$", block)
        if match:
            flush()
            heading = match.group(1).strip()[:200]
            continue
        while len(block) > MAX_CHUNK_CHARS:
            flush()
            output.append((heading, block[:MAX_CHUNK_CHARS]))
            block = block[MAX_CHUNK_CHARS:]
        if length + len(block) + 2 > MAX_CHUNK_CHARS:
            flush()
        pending.append(block)
        length += len(block) + 2
    flush()
    return output


def source_files(root: pathlib.Path, patterns: tuple[str, ...]) -> list[pathlib.Path]:
    files = {path.resolve() for pattern in patterns for path in root.glob(pattern) if path.is_file()}
    return sorted(path for path in files if path.is_relative_to(root.resolve()))


def embed(base_url: str, token: str, model: str, texts: list[str]) -> list[list[float]]:
    response = request_json(
        "POST", f"{base_url.rstrip('/')}/v1/embeddings", {"model": model, "input": texts}, token
    )
    data = response.get("data") if isinstance(response, dict) else None
    if not isinstance(data, list) or len(data) != len(texts):
        raise ValueError("embedding response does not match the input batch")
    ordered = sorted(data, key=lambda item: item.get("index", -1))
    vectors = [item.get("embedding") for item in ordered]
    if any(not isinstance(vector, list) or not vector for vector in vectors):
        raise ValueError("embedding response contains an invalid vector")
    return vectors


def ensure_collection(qdrant: str, collection: str, dimensions: int) -> None:
    url = f"{qdrant.rstrip('/')}/collections/{collection}"
    try:
        response = request_json("GET", url)
    except urllib.error.HTTPError as error:
        if error.code != 404:
            raise
        request_json(
            "PUT",
            url,
            {"vectors": {"size": dimensions, "distance": "Cosine"}, "on_disk_payload": True},
        )
        return
    current = response["result"]["config"]["params"]["vectors"]["size"]
    if current != dimensions:
        raise ValueError(f"collection dimension is {current}, embedding dimension is {dimensions}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=pathlib.Path)
    parser.add_argument("--litellm-url", required=True)
    parser.add_argument("--qdrant-url", required=True)
    parser.add_argument("--token-file", required=True, type=pathlib.Path)
    parser.add_argument("--model", default="nomic-embed-text")
    parser.add_argument("--collection", default="jarvis_knowledge_v1")
    parser.add_argument("--include", action="append", dest="patterns")
    parser.add_argument("--recreate", action="store_true", help="replace only the named collection")
    args = parser.parse_args()

    root = args.root.resolve()
    token = args.token_file.read_text().strip()
    if len(token) < 20 or not re.fullmatch(r"[A-Za-z0-9_.-]+", args.collection):
        raise ValueError("invalid token or collection")

    records: list[dict[str, str]] = []
    for path in source_files(root, tuple(args.patterns or DEFAULT_PATTERNS)):
        if path.stat().st_size > MAX_FILE_BYTES:
            raise ValueError(f"source file is too large: {path}")
        text = path.read_text(encoding="utf-8")
        if SECRET_PATTERN.search(text):
            raise ValueError(f"possible credential found in source: {path}")
        source = path.relative_to(root).as_posix()
        for index, (title, chunk) in enumerate(chunks(text)):
            digest = hashlib.sha256(chunk.encode()).hexdigest()
            records.append({
                "id": str(uuid.uuid5(NAMESPACE, f"{source}:{index}:{digest}")),
                "source": source,
                "title": title,
                "text": chunk,
                "sha256": digest,
            })
    if not records:
        raise ValueError("no documents matched")

    batch_size = 16
    first_vectors = embed(args.litellm_url, token, args.model, [f"{item['title']}\n{item['text']}" for item in records[:batch_size]])
    if args.recreate:
        try:
            request_json("DELETE", f"{args.qdrant_url.rstrip('/')}/collections/{args.collection}")
        except urllib.error.HTTPError as error:
            if error.code != 404:
                raise
    ensure_collection(args.qdrant_url, args.collection, len(first_vectors[0]))
    indexed = 0
    for offset in range(0, len(records), batch_size):
        batch = records[offset : offset + batch_size]
        vectors = first_vectors if offset == 0 else embed(
            args.litellm_url,
            token,
            args.model,
            [f"{item['title']}\n{item['text']}" for item in batch],
        )
        points = [{"id": item["id"], "vector": vector, "payload": item} for item, vector in zip(batch, vectors)]
        request_json(
            "PUT",
            f"{args.qdrant_url.rstrip('/')}/collections/{args.collection}/points?wait=true",
            {"points": points},
        )
        indexed += len(points)
        print(f"indexed={indexed}/{len(records)}", file=sys.stderr)
    print(json.dumps({"collection": args.collection, "points": indexed, "files": len(source_files(root, tuple(args.patterns or DEFAULT_PATTERNS)))}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
