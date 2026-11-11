# Patched TLS dependencies

These sources permit reproducible offline builds while Cargo's registry network
is unavailable. Cargo.toml selects them with `[patch.crates-io]`; Cargo.lock
resolves rustls 0.23.46, rustls-webpki 0.103.16, and webpki-root-certs 1.0.10.

rustls and rustls-webpki start from the cached crates.io 0.23.45 and 0.103.15
packages. Every packaged source/data file changed between their recorded base
commits and the official release tags was replaced with the upstream release
file. Normalized Cargo.toml files differ from the cached manifests only in their
package versions; upstream manifests confirm no other dependency changes.
Cargo.toml.orig contains the exact release manifest. Licenses are retained as
expanded files (upstream rustls uses symlinks to the workspace licenses).

webpki-root-certs contains the exact official 1.0.10 library, README, and license.
Its standalone manifest resolves upstream workspace metadata and the runtime
rustls-pki-types requirement; development-only dependencies are omitted.

Each UPSTREAM.json records authoritative release commits and Git blob hashes.
Run `python3 checks/dependency-vendor-verify.py` to verify shipped upstream files.
Cached Cargo.lock files and Cargo registry markers are omitted. Crate tests may
reference test assets excluded from the original registry packages; application
tests exercise the runtime dependencies instead.

Run `python3 checks/dependency-audit.py` after building the app offline. It scans
all Cargo.lock entries against the pinned RustSec snapshot, including optional
and platform-specific entries, using Rust semver matching. This is not a live
cargo-audit run: refresh the snapshot to discover advisories published later.

## Lua source archive

`lua-5.4.9.tar.gz` is the unmodified official archive from
https://www.lua.org/ftp/lua-5.4.9.tar.gz. Its SHA-256 is
`2335b6c582a52654f94612bf10d2f4672805d05329aa6568b1d8cd9e5c6fb8e6`.
The Windows packaging script verifies this checksum before extracting and
compiling it, so release builds can use the pinned source without downloading
it from lua.org. Lua's MIT license is retained in `runtime/LUA-LICENSE.txt`
and in the archive's documentation.
