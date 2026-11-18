#!/usr/bin/env python3
"""Reject embedded Lua FFI/loader payloads in the actual shipped executable."""
from pathlib import Path
import sys

symbols = ["lua_newstate", "luaL_loadbufferx", "lua_pcallk", "lua54.dll", "liblua5.4.so", "liblua5.4.dylib"]
for argument in sys.argv[1:]:
    path = Path(argument)
    data = path.read_bytes()
    for symbol in symbols:
        if symbol.encode() in data or symbol.encode("utf-16-le") in data:
            raise SystemExit(f"Unexpected Lua runtime payload in {path}: {symbol}")
    print(f"No Lua runtime payload: {path}")
if len(sys.argv) < 2:
    raise SystemExit("Usage: no-lua-runtime.py EXECUTABLE [EXECUTABLE ...]")
