# Native automation and extension compatibility

PixelCraft automation uses its native CLI and MCP interfaces. Lua scripting, Lua plugins, script dialogs and Lua compatibility APIs are no longer supported. There is no replacement in-process scripting engine.

Existing project artwork and supported ASE metadata remain readable. Removing Lua does not change native layers, frames, palettes, references or export identities.

Asset packages may provide themes, palettes, shortcuts, language files and dithering matrices using the documented manifest contributions. Script-only packages cannot be installed/enabled. Mixed packages keep their independent assets and report unsupported script contributions; listing or enabling packages never evaluates scripts. Existing script files/preferences are left intact for manual recovery.

Use [the migration guide and native workflow cookbook](docs/native-automation-migration.md) to replace automation workflows. Third-party editor scripting APIs and executable extension compatibility are not supported; declarative asset compatibility depends on the individual supported formats.
