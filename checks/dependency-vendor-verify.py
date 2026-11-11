#!/usr/bin/env python3
"""Verify shipped upstream files against pinned release Git blob hashes."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent.parent / 'vendor'
for directory in sorted(root.iterdir()):
    if not directory.is_dir():
        continue
    upstream = json.loads((directory / 'UPSTREAM.json').read_text())
    checked = 0
    for entry in upstream['tree']:
        path = directory / ('Cargo.toml.orig' if entry['path'] == 'Cargo.toml' else entry['path'])
        data = path.read_bytes()
        digest = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()
        assert digest == entry['sha'], f'Upstream source mismatch: {path}'
        checked += 1
    assert checked, directory
    print(f'{directory.name}: {checked} files match {upstream["release_commit"]}')
