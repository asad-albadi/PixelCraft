# Workspace restructuring verification — issue #27

Baseline: v0.11.0, `46576e1c15009e79bbc1021fb3321f7a100ce0f6`.
Scope: repository and architecture refactor; no Android, Flutter, FFI, UI redesign
or editor feature additions. Rust checks use 1.95.0 and isolated application data.

## Batches and reviews

| Batch | Evidence |
| --- | --- |
| Baseline | 806 tests passed; 16 opt-in tests excluded |
| Core and guarded storage | 806 tests passed; strict all-target workspace Clippy passed |
| Shared operations, CLI and MCP | 806 tests passed; strict lint; CLI help and complete MCP tool schemas matched the release |
| Desktop relocation | 806 tests passed; strict lint; ordinary root build and dependency boundaries passed |
| Shared editor session | 811 tests passed; strict lint; independent session/history/selection/stroke regressions added |
| Final integration | Full workspace suite, native checks and hosted results recorded below |

Independent reviews ran after each major extraction and across the branch for
both code and security. Findings addressed before proceeding:

- Keep the single storage guard and staging registry below the codecs; do not
  compile duplicate implementations or introduce a core-to-MCP dependency.
- Preserve the codec runtime broker's private visibility.
- Move cross-layer command tests to the compatibility integration suite and
  deadline/transport tests to MCP rather than adding reverse dependencies.
- Include moved unit/security tests in default workspace checks; update native
  UI/clipboard selectors to the desktop library so they cannot pass with no tests.
- Preserve desktop author metadata and the initial selected tile of 1.
- Preserve active-tag invalidation at every intermediate Undo/Redo jump state.
  The new tag-crossing regression failed before the observer fix and passed after.

The final independent source reviews reported no remaining actionable findings.
They are code reviews, not runtime penetration tests.

## Executable and visual equivalence

Compared the released v0.11.0 AppImage with the refactored executable using
separate temporary user-data directories and no graphical display for automation:

- CLI help text matches exactly.
- MCP initialize responses and complete tool schemas match exactly.
- Native create/edit files and PNG, GIF, ASE and sprite-sheet PNG/JSON export
  bytes match exactly for the same small animation workflow.
- Existing `checks/productivity-headless.py` passes with display variables removed.
- All three opt-in productivity live-bridge tests pass: authentication/opt-in,
  deadlines/revocation, and export progress/cancellation on a second connection.
- Independent core-only tests pass without desktop or transport dependencies.
- Native clipboard and X11 packed-tilemap cross-process tests pass on the isolated display.
- `checks/standalone.sh` passes with a restricted filesystem and no installed
  editor assets: drawing, native save/reopen, PNG/sheet/GIF, native commands,
  theme, palette and shortcuts, extension disable/uninstall.

Native Linux captures of the released and refactored demo editor were produced
on an isolated X server at 1920×1080 using software OpenGL. Light and dark images
match pixel-for-pixel and were inspected visually. Inspector, tools, timeline and
bottom-left Preview retain their placement. These captures do not test physical
monitor changes or a window manager's titlebar decorations.

Local screenshots, comparison metadata and dialog captures are retained outside
the source tree under the task's `outputs/issue27-workspace-validation` directory.
README screenshots remain unchanged because the interface is unchanged.

## Dependency evidence and limitation

All external Cargo dependency versions, sources and checksums match the baseline.
The five new package names are internal workspace packages. All package versions
inherit one workspace version. `checks/workspace-boundaries.py` verifies the
single executable, complete default test membership, aligned versions and absence
of GUI/transport dependencies from shared layers.

The optional offline `checks/dependency-audit.py` **does not pass**: its pinned
advisory snapshot lacks 57 external package names already present at the baseline.
The missing external names are identical before and after restructuring. Only
the five internal package names were added to its inventory; advisory records
were not refreshed. This is a pre-existing coverage limitation, not evidence of
a fresh advisory scan or a newly introduced vulnerable dependency.

## Final checks

- [x] Final workspace formatting and strict all-target Clippy pass; full suite: 811 passed, 0 failed, 16 opt-in ignores.
- [x] Four dialog checks pass; 64 captures cover Settings and focused destinations at compact/Full HD sizes, 100–200% synthetic scaling and both themes. Representative captures were inspected.
Hosted Linux, Windows, Apple Silicon and Intel macOS checks, and the seven-package
preview, are tracked in the associated pull request and its Actions runs. Consult
those commit-specific results for platform acceptance; Linux results do not
substitute for native Windows/macOS execution.

No release tag or publication is part of this issue.
