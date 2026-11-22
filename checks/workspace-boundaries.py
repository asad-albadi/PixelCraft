#!/usr/bin/env python3
"""Check the reusable engine cannot acquire UI or transport dependencies."""
import json
import subprocess

metadata = json.loads(subprocess.check_output(
    ["cargo", "+1.95.0", "metadata", "--locked", "--format-version", "1"], text=True))
packages = {p["id"]: p for p in metadata["packages"]}
members = {packages[i]["name"]: i for i in metadata["workspace_members"]}
required = {"pixelcraft", "pixelcraft-core", "pixelcraft-automation",
            "pixelcraft-cli", "pixelcraft-mcp", "pixelcraft-desktop", "pixelcraft-ffi"}
assert required <= members.keys(), f"Missing workspace members: {required - members.keys()}"
versions = {packages[i]["version"] for i in metadata["workspace_members"]}
assert len(versions) == 1, f"Workspace package versions drifted: {versions}"
assert set(metadata["workspace_default_members"]) == set(metadata["workspace_members"]), "Default checks would skip workspace members"
binaries = [(packages[i]["name"], t["name"]) for i in metadata["workspace_members"]
            for t in packages[i]["targets"] if "bin" in t["kind"]]
assert binaries == [("pixelcraft", "pixelcraft")], f"Public executable layout changed: {binaries}"
edges = {n["id"]: n["dependencies"] for n in metadata["resolve"]["nodes"]}

def reachable(start):
    seen, todo = set(), [start]
    while todo:
        node = todo.pop()
        if node not in seen:
            seen.add(node)
            todo.extend(edges[node])
    return {packages[i]["name"] for i in seen}

for name in ("pixelcraft-core", "pixelcraft-automation", "pixelcraft-cli", "pixelcraft-mcp", "pixelcraft-ffi"):
    deps = reachable(members[name])
    forbidden = {"eframe", "egui", "arboard", "rfd", "pixelcraft-desktop", "pixelcraft"}
    assert not deps & forbidden, f"{name} pulls in desktop dependencies: {deps & forbidden}"
assert not reachable(members["pixelcraft-core"]) & {"pixelcraft-mcp", "pixelcraft-cli", "pixelcraft-automation"}
assert not reachable(members["pixelcraft-automation"]) & {"pixelcraft-mcp", "pixelcraft-cli"}
print("Workspace engine and adapter dependency boundaries verified")

assert not reachable(members["pixelcraft-ffi"]) & {"pixelcraft-cli", "pixelcraft-mcp", "pixelcraft-automation"}
