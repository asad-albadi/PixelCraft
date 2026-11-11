"""End-to-end CLI and MCP checks, deliberately without a graphical display."""
import os, json, subprocess, tempfile, pathlib, base64
root = pathlib.Path(tempfile.mkdtemp(prefix='pixelcraft-headless-'))
binary = str(pathlib.Path('target/debug/pixelcraft').resolve())
env = dict(os.environ)
env.pop('DISPLAY', None)
env.pop('WAYLAND_DISPLAY', None)
def cli(op, **args):
    p = subprocess.run([binary, '--cli', op, json.dumps(args)], env=env, capture_output=True, text=True)
    assert p.returncode == 0, (p.stderr, p.stdout)
    return json.loads(p.stdout)
source = str(pathlib.Path('tests/fixtures/rendered/2f-index-3x3.png').resolve())
project = str(root / 'sliced.pixel')
original = cli('inspect', input=source)
grid = cli('import-grid', input=source, output=project, options={'width': 1, 'height': 1})
assert len(grid['frames']) == original['width'] * original['height']
info = cli('inspect', input=project)
updated = str(root / 'edited.pixel')
cli('edit', input=project, output=updated, revision=info['revision'], edits=[{'layer': 0, 'frame': 0, 'x': 0, 'y': 0, 'color': [8, 16, 32, 255]}])
cli('export', input=updated, output=str(root / 'sheet.png'), format='sheet', options={'preset': 'Unity', 'columns': 3})
assert (root / 'sheet.png.meta').exists()
cli('atlas', inputs=[project, updated], output=str(root / 'atlas.png'), options={'layout': 'MaxRects', 'power_of_two': True, 'width': 64})
cli('palette-export', input=updated, output=str(root / 'colors.PAL'))
cli('palette-convert', input=str(root / 'colors.PAL'), output=str(root / 'colors.hex'))
tagged = str(pathlib.Path('tests/fixtures/tags3x123reps.aseprite').resolve())
batch = cli('batch-tags', input=tagged, output=str(root / 'tags'), format='sheet', options={'preset': 'Godot'})
assert batch['outputs']
requests = [
    {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params': {'protocolVersion': '2024-11-05', 'capabilities': {}, 'clientInfo': {'name': 'test', 'version': '1'}}},
    {'jsonrpc': '2.0', 'method': 'notifications/initialized'},
    {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'},
    {'jsonrpc': '2.0', 'id': 3, 'method': 'tools/call', 'params': {'name': 'render', 'arguments': {'input': updated}}},
]
p = subprocess.run([binary, '--mcp'], input='\n'.join(json.dumps(r) for r in requests) + '\n', capture_output=True, text=True, env=env)
assert p.returncode == 0, p.stderr
responses = [json.loads(line) for line in p.stdout.splitlines()]
assert len(responses) == 3
content = responses[2]['result']['content'][0]
assert content['type'] == 'image'
assert base64.b64decode(content['data']).startswith(b'\x89PNG')
assert not any(t['name'].startswith('live-') for t in responses[1]['result']['tools'])
print('PASS: CLI grid import, inspect, revision-checked edits, Unity export, MaxRects atlas, uppercase PAL/HEX conversion, batch Godot tag sheets.')
print('PASS: MCP initialize/notification/tools/render PNG and live tools disabled. DISPLAY and WAYLAND_DISPLAY removed.')
print('Artifacts:', root)
