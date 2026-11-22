#!/usr/bin/env python3
"""Check the reusable engine cannot acquire UI or transport dependencies."""
import json
import subprocess

metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--locked", "--format-version", "1"], text=True))
packages = {p["id"]: p for p in metadata["packages"]}
members = {packages[i]["name"]: i for i in metadata["workspace_members"]}
required = {"pixelcraft", "pixelcraft-core", "pixelcraft-automation",
            "pixelcraft-cli", "pixelcraft-mcp", "pixelcraft-desktop"}
assert required <= members.keys(), f"Missing workspace members: {required - members.keys()}"
edges = {n["id"]: n["dependencies"] for n in metadata["resolve"]["nodes"]}

def reachable(start):
    seen, todo = set(), [start]
    while todo:
        node = todo.pop()
        if node not in seen:
            seen.add(node)
            todo.extend(edges[node])
    return {packages[i]["name"] for i in seen}

for name in ("pixelcraft-core", "pixelcraft-automation", "pixelcraft-cli", "pixelcraft-mcp"):
    deps = reachable(members[name])
    forbidden = {"eframe", "egui", "arboard", "rfd", "pixelcraft-desktop", "pixelcraft"}
    assert not deps & forbidden, f"{name} pulls in desktop dependencies: {deps & forbidden}"
assert not reachable(members["pixelcraft-core"]) & {"pixelcraft-mcp", "pixelcraft-cli", "pixelcraft-automation"}
assert not reachable(members["pixelcraft-automation"]) & {"pixelcraft-mcp", "pixelcraft-cli"}
print("Workspace engine and adapter dependency boundaries verified")
