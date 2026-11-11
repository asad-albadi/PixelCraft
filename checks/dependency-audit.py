#!/usr/bin/env python3
"""Offline RustSec lockfile scan against the pinned, integrity-checked snapshot.

Uses Rust's semver crate for Cargo version requirements. This is a snapshot
scan, not cargo-audit, and does not fetch new advisories. Refresh the snapshot
when Cargo.lock adds package names or when checking for newly published issues.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent
SNAPSHOT = ROOT / 'checks/dependency-advisories'
manifest = json.loads((SNAPSHOT / 'snapshot.json').read_text())
packages = tomllib.loads((ROOT / 'Cargo.lock').read_text())['package']
assert set(p['name'] for p in packages) <= set(manifest['locked_names']), 'Refresh snapshot for new lockfile packages'
advisories = []
for entry in manifest['files']:
    data = (SNAPSHOT / entry['path']).read_bytes()
    digest = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()
    assert digest == entry['sha'], f"Snapshot integrity mismatch: {entry['path']}"
    metadata = tomllib.loads(data.decode().split('```toml', 1)[1].split('```', 1)[0])
    advisories.append(metadata)

# Compile a small helper with the same semver implementation used by Cargo.
# Build the app once offline first if its semver rlib is not available.
libraries = sorted((ROOT / 'target/debug/deps').glob('libsemver-*.rlib'))
assert libraries, 'Run cargo build --offline first to compile semver'
source = '''use std::io::{self, BufRead};
fn main() {
 for line in io::stdin().lock().lines() {
  let line = line.unwrap(); let (v, r) = line.split_once('\\t').unwrap();
  let version = semver::Version::parse(v).unwrap();
  let req = semver::VersionReq::parse(r).unwrap();
  println!("{}", req.matches(&version));
 }
}'''
comparisons = []
checks = []
withdrawn = []
for advisory in advisories:
    info = advisory['advisory']
    if info.get('withdrawn'):
        withdrawn.append(info['id'])
        continue
    for package in packages:
        if package['name'] != info['package']:
            continue
        ranges = advisory.get('versions', {}).get('patched', []) + advisory.get('versions', {}).get('unaffected', [])
        start = len(comparisons)
        comparisons.extend(package['version'] + '\t' + requirement for requirement in ranges)
        checks.append((package, info, start, len(comparisons)))
with tempfile.TemporaryDirectory(prefix='pixelcraft-dependency-audit-') as tmp:
    helper = Path(tmp) / 'matches'
    rust = Path(tmp) / 'matches.rs'
    rust.write_text(source)
    subprocess.run(['rustc', str(rust), '--edition=2024', '--extern', 'semver=' + str(libraries[0]), '-L', 'dependency=' + str(ROOT / 'target/debug/deps'), '-o', str(helper)], check=True)
    result = subprocess.run([str(helper)], input='\n'.join(comparisons) + '\n', text=True, capture_output=True, check=True)
    matches = result.stdout.splitlines()
assert len(matches) == len(comparisons)
vulnerabilities, warnings = [], []
for package, info, start, end in checks:
    if any(value == 'true' for value in matches[start:end]):
        continue
    finding = {'id': info['id'], 'package': package['name'], 'version': package['version'], 'url': 'https://rustsec.org/advisories/' + info['id'] + '.html'}
    (warnings if info.get('informational') else vulnerabilities).append(finding)
report = {'scanner': 'pinned RustSec snapshot with Rust semver matching (not cargo-audit)', 'database_commit': manifest['commit'], 'snapshot_date': manifest['retrieved'], 'lock_packages': len(packages), 'advisories_considered': len(advisories), 'withdrawn_skipped': withdrawn, 'vulnerabilities': vulnerabilities, 'informational_warnings': warnings}
print(json.dumps(report, indent=2))
raise SystemExit(bool(vulnerabilities))
