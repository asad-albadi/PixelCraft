#!/usr/bin/env python3
"""Export the real built-in garden via PixelCraft's desktop-hosted Rust C ABI.
Usage: python3 export-sample.py /absolute/path/to/libpixelcraft_ffi.so
"""
import ctypes as C
import json
from pathlib import Path
import sys
lib=C.CDLL(str(Path(sys.argv[1]).resolve()))
lib.pc_request.argtypes=[C.c_uint64,C.c_uint32,C.c_uint32,C.c_uint32,C.c_uint64]
lib.pc_request.restype=C.c_uint64
lib.pc_buffer_data.argtypes=[C.c_uint64];lib.pc_buffer_data.restype=C.c_void_p
lib.pc_buffer_len.argtypes=[C.c_uint64];lib.pc_buffer_len.restype=C.c_size_t
lib.pc_buffer_kind.argtypes=[C.c_uint64];lib.pc_buffer_kind.restype=C.c_uint32
lib.pc_buffer_free.argtypes=[C.c_uint64]
lib.pc_session_close.argtypes=[C.c_uint64]
def request(session,op,a=0):
 h=lib.pc_request(session,op,a,0,0)
 if not h: raise RuntimeError('Engine returned no buffer')
 try:
  data=C.string_at(lib.pc_buffer_data(h),lib.pc_buffer_len(h))
  if lib.pc_buffer_kind(h)==0: raise RuntimeError(data.decode())
  return data
 finally: lib.pc_buffer_free(h)
state=json.loads(request(0,1))
try:
 data=request(state['session'],24,1)
 assert data.startswith(b'\x89PNG\r\n\x1a\n')
 Path(__file__).with_name('garden-sample.png').write_bytes(data)
finally:lib.pc_session_close(state['session'])
