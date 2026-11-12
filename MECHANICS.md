## Fresh native audit: Point assignment (2026-11-12)

Opened the original main ELF using `core_target_open`: `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Session confirmed Deep mode.

Searched names, settings and messages with `native_strings_find`: Point/x/y (`ev_df96af5d6c1c6ea9e71b222e771e9ec98bb815ea94529a4f4b38d3e3f7f65ab0`) and color_bar/invalid_fg_or_bg (`ev_50ec939f374744c4ff1e2658b54904d4bedd7e410e817e53c61db85b04ad6198`). Point string `0xf409c0` leads to registration `0x3e4790` and property table `0x1cbc700`; setter pointer `0x1cbc710` identifies x setter. Reference evidence: `ev_531d3a07731d6f46178762cf149ddf345245882b478ee6c25bc537f199412729`, `ev_1f6d25e1881fad0bef6e2389976e6d9fa00f812744cc1f4b292d82b3b705846b`, `ev_e22571c92cd4f29da76a2285c52cd3d830e3ccbaff58ccf9b2d9173627d27bee`, `ev_3e35e40d474dcd4a39367e8725bb7ed46efb12db236860a3e071307e6d4b04b6`. Settings/messages serve as navigation; no Point-specific validation message was established.

Each `native_function_pseudocode` call was individually awaited and read before requesting the next:

| Function | Address | Recovered rule | Evidence |
| --- | --- | --- | --- |
| Point_set_x | `0x3e5050` | Convert argument 2; store low32 at offset0 | `ev_82bac048a09a2638f436922dd300c3ebd9a9277276a3d45c2605807e930d9935` |
| Point_set_y | `0x3e50c0` | Convert argument 2; store low32 at offset4 | `ev_a25b57eb0a9804b51d51f46b801025264c5cd8d36feb3ff8fe1c1c26a6f7062f` |
| lua_tointegerx | `0x53eab0` | Integer identity, helper mode1 otherwise; failure zero | `ev_9c6b8f650a06d892e72db127b3c955fa69744b7fe82cbde819b808074d3d9ff8` |
| luaV_tointeger | `0x554930` | Parse numeric strings; floor floats within signed64 range | `ev_56f35559cf54f97b836849e968df1af58f3abd51a8ede842b1d0cccfaa4df2c3` |

Formula: I(v) preserves Lua integers, parses numeric strings, floors floating inputs in [-2^63,2^63), and returns zero on failed conversion. For u=I(v)&0xffffffff, store u when u<2^31, otherwise u-2^32. Thus -1.1 becomes -2, '2.9' becomes 2, 4294967297 becomes 1, and 2147483648 becomes -2147483648. Nil, booleans, tables, invalid strings, NaN and infinity become zero. Coordinates are not clamped to canvas bounds; each setter preserves the other coordinate.

`src/lua_api.lua` now keeps Point coordinates behind a proxy so every write converts, including repeated writes and writes after nil. Construction and independent copies retain their previous selection rules. `src/lua_compat.lua` serializes canonical x/y for host requests. Embedded Lua tests in `tests/native_point_setters.rs` exercise conversion and real Image:drawImage pixel placement.

Engine limits: Ghidra timed out under a temporary 45-second startup limit. These four completed results are built-in fallback pseudocode, not successful Ghidra decompilation. Restored the startup setting to null. Setter assembly in [point_setters.asm](evidence/point_setters.asm) confirms conversion calls and 32-bit stores; helper assembly in evidence/lua_tointegerx.asm and evidence/luaV_tointeger.asm corroborates approximate helper lifting. Raw completed results are in [point_setters.json](evidence/point_setters.json). Native userdata identity, Point operators and whole-application equivalence remain unverified.

Validation: both new regressions fail on the previous Point implementation and pass with the correction. Full `cargo test --offline --locked` passed 225 tests, 0 failed, 3 ignored; the final strengthened pixel-placement test also passed in a targeted run. `cargo fmt --check` and `cargo build --offline --locked` passed. Logs: checks/native-point-setters-red.log, checks/native-point-setters-green.log, checks/native-point-setters-tests.log, checks/native-point-setters-build.log. Craft Anything rebuild_check passed build, startup/render (942×1005) and mechanics (44 values, 43 sourced, one estimated); screenshot inspected. Report: checks/native-point-setters-rebuild-check.json.

## Fresh native audit: Size field assignment (2026-11-12)

Opened the original main ELF with `core_target_open`: `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Session confirmed Deep mode.

`native_strings_find` searched names, settings and messages (`^Size$|^width$|^height$|^grid_bounds$|invalid_fg_or_bg`), evidence `ev_578a341abdafd1e1476316faa246ea505b51aeaa34692358eac3f9bf4ae28fb9`. Size string `0xf412df` references registration `0x3ef1d0` (`ev_9e003ef988c62bf1b2ec5798adf5235e0913b4d6396e46c2832a04a46b46f560`), whose outgoing references identify property table `0x1cbcff0` (`ev_bb0bb7344ad3ae01f7949911a2c2e1539c5209eb2a4560111194f3190b15233d`). Setter pointer references at `0x1cbd000` and `0x1cbd030` establish shared width aliases (`ev_b7925d4a323432597f7276ec3b2e52974de0ccd2f61e2cac89ca33c3b12150a2`). Outgoing calls establish integer conversion (`ev_21048315601e6a155832dc633e212829d28cceb1505324cbf4e0423bf605eea2`). Settings and messages are navigation evidence; no Size-specific validation message was established.

Requested, individually awaited to completion, and read each `native_function_pseudocode` result before requesting the next:

| Function | Address | Recovered rule | Evidence |
| --- | --- | --- | --- |
| Size_set_width | `0x3efb20` | Argument 2 converted; low 32 bits stored at offset 0 | `ev_8f5d05eca6f0728fe79112fa6a6c7e973011541057d3c7ca7105c13a20dd9437` |
| Size_set_height | `0x3efb90` | Same, offset 4 | `ev_92410839d97cbca6dd1bbfb27f6d0e2048be2106cc23f4e7658bb1163493dc96` |
| lua_tointegerx | `0x53eab0` | Integer identity; otherwise helper mode 1; failed conversion returns initialized zero | `ev_86b623a68e1d42351968a04f3f8029f5cafa353a8c8d48c23edc3dad10a32bee` |
| luaV_tointeger | `0x554930` | Numeric strings parsed; finite floats floored in mode 1 within signed64 range | `ev_a57ebda955ad38537b3fa2b4b7f5179abd0d37d43a58810b4a664408d4f96433` |

Formula: `I(v)` preserves integer inputs, parses numeric strings, floors finite floating inputs in `[-2^63, 2^63)`, and gives zero for failed conversion. Let `u = I(v) & 0xffffffff`; store `u` when `u < 2^31`, otherwise `u - 2^32`. Thus `-1.1 -> -2`, `'2.9' -> 2`, `4294967297 -> 1`, `2147483648 -> -2147483648`; nil, boolean, invalid strings, NaN and infinity yield zero. Negative dimensions remain negative; setters apply no positivity or canvas clamp. `w` and `width` share one field, `h` and `height` share the other. Property-table bytes and strings at `0xf58609` (`w`), `0xf621ce` (`h`), `0xf60c26` (`width`), `0xf60c39` (`height`) corroborate alias registration.

Corrected `src/lua_api.lua` to keep Size fields behind a proxy and convert every assignment, including repeated writes and writes after nil. Aliases share canonical storage; copies remain independent. `src/lua_compat.lua` serializes the canonical fields for host requests. Constructor field selection is retained. Two real embedded-Lua regressions in `tests/native_size_setters.rs` failed before the correction: unconverted assignments and rejected image dimensions from a numeric string. They also cover signed32 wrapping, failed conversions, aliases, copying and actual image pixel access.

Engine limitation: Ghidra timed out at a temporary 45-second startup deadline; all four completed pseudocode results came from the built-in fallback lifter, not successful Ghidra decompilation. The startup setting was restored to null. Raw results are in [size_setters.json](evidence/size_setters.json); native setter assembly is in [size_setters.asm](evidence/size_setters.asm). Helper assembly already retained in [luaV_tointeger.asm](evidence/luaV_tointeger.asm) and [lua_tointegerx.asm](evidence/lua_tointegerx.asm) corroborates the approximate helper lifting. Full userdata identity, Size operators, Point mutation and whole-application parity remain unverified.

Validation: both new regressions failed before correction and passed afterward. Fresh `cargo test --offline --locked` with `PIXELCRAFT_HOME=/tmp/pixelcraft-size-setters` passed 217 tests, zero failed, two ignored loopback tests. `cargo fmt --check` and `cargo build --offline --locked` passed. Logs: `checks/native-size-setters-red.log`, `checks/native-size-setters-tests.log`, `checks/native-size-setters-build.log`. Craft Anything `rebuild_check` passed build, startup/render (942×1005) and mechanics (43 values, 42 sourced, one estimated); screenshot visually inspected. Report: `checks/native-size-setters-rebuild-check.json`.

## Fresh native audit: Rectangle field assignment (2026-11-12)

Opened the original main ELF with `core_target_open` at `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Searched names, settings and messages using `native_strings_find`: `ev_b41af2ed83151b0475179848793ce3909e10476fb5e05b6cd8f61324f6fa5a68` and `ev_7aa46590093769f686d4cadba73f5f76e1c80a97838fa52bfaa8ff4188b2bae8`. The Rectangle string at `0xf40c0a` leads to registration `0x3e9d40`, then property table `0x1cbca40`; setter pointer references and outgoing calls are recorded in `ev_02d6b563fbf48802fe3de44285141616337bb3691c203a2a985ee566fbbd0465`, `ev_d6fef4322563644099449073c6125240bfa30e6e6f42e7efe3e1249463cd5c2e` and `ev_2c2585352514ac8c14b938279c32e1f781f8f8ce8bf75fb141e2dfc441b85fcd`. Message search did not establish a rectangle-specific validation message; no such rule was invented.

Each of these six `native_function_pseudocode` calls was individually awaited to completion and read before requesting the next:

| Function | Address | Recovered rule | Evidence |
| --- | --- | --- | --- |
| Rectangle_set_x | `0x3ea6d0` | Convert argument 2; store low 32 bits at offset 0 | `ev_b58774499b6dbb8070ea606e3944ce94d1c1afa77d78aa1a5fadce3bba7808a2` |
| Rectangle_set_y | `0x3ea740` | Same, offset 4 | `ev_12c323ac5b99b5b149e7aae033911e59d348bf998e000ff72688b540a50204ac` |
| Rectangle_set_width | `0x3ea7b0` | Same, offset 8 | `ev_c1ef3e7b8362daabe50633874aa0a6a2d2a00fa0d2a1353f51f54840ec751b66` |
| Rectangle_set_height | `0x3ea820` | Same, offset 12 | `ev_f2dd0d725091c90a849c49e4cf59460f65de00cea916b3157b1d8702156ff9b1` |
| lua_tointegerx | `0x53eab0` | Zero result initially; integer identity; otherwise helper mode 1 | `ev_dd4c3f99a510b1ed5f1bf83f19090805b925fbd703b6729545ff1ad64994cad4` |
| luaV_tointeger | `0x554930` | Parse numeric strings; floor floating values in mode 1; require signed64 range | `ev_1d294ffa1b28a6c31b9cc4b77d72801898b8fc48006492e4f491eb560c1de59b` |

Formula for every canonical field assignment: let `I(v)` preserve integers, parse numeric strings, floor finite floating inputs in `[-2^63,2^63)`, and return zero on conversion failure. Let `u = I(v) & 0xffffffff`; store `u` if `u < 2^31`, otherwise `u - 2^32`. Thus `-1.1 -> -2`, `'2.9' -> 2`, `4294967297 -> 1`, `2147483648 -> -2147483648`; nil, booleans, invalid strings, NaN and infinity convert to zero. Negative dimensions are preserved. Setters perform no positivity or canvas-bound clamp and leave the other components untouched.

Changed `src/lua_api.lua` from plain Rectangle fields to fields behind a proxy that converts every write, including repeated writes and writes after nil. Existing constructor selection and readable canonical fields are preserved; `pairs` exposes canonical fields for copying. `tests/native_rectangle_setters.rs` reproduced both the conversion failure and incorrect Image pixel coverage before the correction.

Validation: both new regressions failed before the fix and passed afterward. The full `cargo test --offline --locked` run passed 215 tests with zero failures using a temporary `PIXELCRAFT_HOME` to keep preferences inside writable storage. An initial unrestricted-settings run failed on a read-only user settings path; after redirecting settings, the suite exposed a Rectangle host-serialization regression, which was corrected in `src/lua_compat.lua` and verified by the existing interactive-selection test. Formatting check and `cargo build --offline --locked` passed. Logs: `checks/native-rectangle-setters-tests.log`, `checks/native-rectangle-setters-build.log`. Craft Anything `rebuild_check` passed build, startup/render (938×990) and mechanics (42 values, 41 sourced, 1 estimate); screenshot inspected. Report: `checks/native-rectangle-setters-rebuild-check.json`.

Engine limits: Deep mode was enabled, but Ghidra exceeded a temporary 15-second startup deadline. All six completed pseudocode results came from the built-in fallback lifter, not successful Ghidra analysis. Its helper output contains approximate conditions and inline assembly; full objdump corroboration is saved in `evidence/rectangle_setters.asm` and all six raw results in `evidence/rectangle_setters.json`. The startup setting was restored to null. This correction covers x/y/width/height assignments only; native geometry userdata identity, aliases, operators, origin/size properties and full application parity remain outside the verified coverage.

## Native audit — Rectangle construction (2026-11-12)

Opened the original main program with `core_target_open`: `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Session confirmed Deep mode.

Searched names, settings and messages using `native_strings_find` regex `^Rectangle$|^width$|^height$|^grid_bounds$|^color_bar$|invalid_fg_or_bg` (seven matches, `ev_b823446da65ab34b63a8c39e115ba12ec2c14975a50bd9684ea214ae4369b0d5`). Followed Rectangle string `0xf40c0a` to registration `0x3e9dcb` (`ev_dc98625f0c70e71a0290281787c0444aed92da255651aaeebc85cf27c4e152f2`); constructor pointer at `0x3e9dba` identifies wrapper `0x3e9e10` (`ev_bf489fbca230bcf5e0655cdbc9e895cd657473bc51e2f4cee212420d8ec80696`). Width reference `0x3e9f22` resolves helper entry `0x3e9ea0` (`ev_b28f8220018263526927fab210147dd692d035167baaf0cbba756d6cf03a2ae2`); outgoing references identify integer conversion (`ev_fd4331015428f2aa0fdcb22d8432fef3faf8d9607099420f854467ef8a31f136`). Settings/messages are navigation evidence only.

Requested, awaited and read each completed `native_function_pseudocode` separately before requesting the next:

| Function | Address | Evidence |
| --- | --- | --- |
| Rectangle_new helper | `0x3e9ea0` | `ev_6b3d47bb493b09ced45c30fe34ad1cb2acb08902a8a00ac9daa3f703bd241dc6` |
| lua_tointegerx | `0x53eab0` | `ev_dd4c3f99a510b1ed5f1bf83f19090805b925fbd703b6729545ff1ad64994cad4` |
| luaV_tointeger | `0x554930` | `ev_1d294ffa1b28a6c31b9cc4b77d72801898b8fc48006492e4f491eb560c1de59b` |
| Rectangle_new wrapper | `0x3e9e10` | `ev_78b4c4c19fe571f5ed07eba3b3ea5f1a6a0336901226ed9fd6076159786a7c54` |

The first pseudocode call timed out at the tool transport after 300 seconds without returning pseudocode. Retried with temporary Ghidra startup timeout 15 seconds; Ghidra timed out and the tool returned completed built-in fallback pseudocode. Restored the original null startup timeout. These are **not successful Ghidra decompilations**. Fallback arguments, flags and SIMD are approximate; [full original helper assembly](evidence/rectangle_constructor.asm) corroborates the rules, branches and 32-bit stores. In particular the assembly preserves the third scalar result in RBP before its 32-bit store; the fallback prints an ambiguous register alias. Complete returned results/limitations are retained in [rectangle_constructor.json](evidence/rectangle_constructor.json).

Recovered rules applied in `src/lua_api.lua`:

- Let **I(v)** preserve Lua integers, parse complete numeric strings, and floor floating numbers within signed64 range. Failed conversion, NaN and infinity return **0**. Each field stores **S(v)=signed32(I(v) mod 2^32)**. Scalar **Rectangle(-1.1,"2.9",4294967297,2147483648)=(-2,2,1,-2147483648)**. Omitted scalar fields become zero; negative dimensions are not clamped.
- For a table, **x nonnil** selects named coordinates **x/y**. Otherwise read **[1]/[2]/[3]/[4]**, ignoring all named fields. With named coordinates, **width nonnil** selects **width/height**; otherwise select **w/h**. False and zero count as nonnil. Missing or invalid values become zero, without cross-family fallback. Thus **Rectangle{x=1,y=2,w=3,h=4}=(1,2,3,4)**, **Rectangle{y=9,width=8,height=7,1,2,3,4}=(1,2,3,4)**, and **Rectangle{x=false,y=2,width=false,height=1,w=8,h=9}=(0,2,0,1)**.
- Helper assembly `0x3e9ecb..0x3e9ece` copies 16 bytes of existing Rectangle userdata. The wrapper allocates **16 bytes** and copies four signed32 fields. Reconstruction copies its four geometry fields into an independent Lua table.

Two real embedded-Lua regressions in `tests/native_rectangle_constructor.rs` failed before correction: fractional/large scalar fields remained unconverted, and short w/h fields caused Image:pixels to read a nil width (`checks/native-rectangle-red.log`). The pixel iteration regression checks actual coverage for short fields, array selection, false/nil precedence and numeric strings. Post-correction results are in `checks/native-rectangle-green.log`.

Verification: full `cargo test --offline --locked` with isolated writable XDG data/config directories passed **210 tests, zero failed, 2 ignored**, exit 0 (`checks/native-rectangle-tests.log`). `cargo fmt --check` and `cargo build --offline --locked` passed. Craft Anything `rebuild_check` passed build, native startup/drawing (**942×1005**) and mechanics validation (**41 values, 40 sourced, one estimated**); inspected its desktop screenshot. Report saved in `checks/native-rectangle-rebuild-check.json`.

Scope: this audit corrects scalar/table Rectangle construction and copying of ordinary Rectangle results. Native userdata identity, property setters/operators and the native Point+Size userdata overload remain unresolved; the build continues to represent geometry with tables. Earlier drawing, color, timing and geometry audits below retain their stated limits. This is not a claim of full application equivalence.

## Native audit — Size constructor dimensions (2026-11-11)

Freshly opened the original main ELF using `core_target_open`: `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`, in Deep mode. This extends the earlier drawing, timing, color and geometry audits retained below.

`native_strings_find` searched names, fields and messages with `^Size$|^width$|^height$|^grid_bounds$|invalid_fg_or_bg` (`ev_b873a7c92a20b14aa1ddba2989fa7315fc36ba2838edfe2e9e27f8e90171b67a`). Size at `0xf412df` references registration at `0x3ef25b`; width/height references at `0x3ef36c` / `0x3ef381` resolve to helper entry `0x3ef330` (`ev_0588995743ddb03f1166363e0742f825b61cc04173836f8225837776daa8065a`). Followed incoming references to the wrapper (`ev_245fac78498f6800afd9d416d999381aaa9c4c2002407b967a34e85db67e2d1a`) and outgoing calls to integer conversion (`ev_53e9665e6f9d17575ab4e7934e0ecbfa2800436882d1b0cf69e79575438f7399`). A separate `^color_bar$` setting search returned `ev_36f1c6671cc15d8200c204259a45d4a52982481a894d79ee86078131352e6cf3`. Settings/messages are navigation evidence only.

Requested, awaited and read each `native_function_pseudocode` result separately before requesting the next:

| Function | Address | Evidence |
| --- | --- | --- |
| `Size_new(lua_State*,int)` helper | `0x3ef330` | `ev_3a75063906a1e7dbdff274872f231897f27cd5d858e3455f09a908c7212caa90` |
| `lua_tointegerx` | `0x53eab0` | `ev_dd4c3f99a510b1ed5f1bf83f19090805b925fbd703b6729545ff1ad64994cad4` |
| `luaV_tointeger` | `0x554930` | `ev_1d294ffa1b28a6c31b9cc4b77d72801898b8fc48006492e4f491eb560c1de59b` |
| `Size_new(lua_State*)` wrapper | `0x3ef2a0` | `ev_48410362017a6ebcd5f0a68fc5355af32bcf1115e58308940c3088e8d808622a` |

Ghidra timed out under a temporary 15-second startup limit. All four completed results use the built-in fallback decompiler, with approximate register arguments/flags and incomplete SIMD recovery; they are not successful Ghidra analysis. Restored the original null timeout. Full helper assembly (`ev_95d0a5ace6f74dde9f2ccba68aa9849ee9d3a0c53f60c50384b4b2833ea206c0`) corroborates field branches and 32-bit stores. Bytes `77 00` at `0xf58609` confirm the short `w` key (`ev_94e01e2af940186a5ae75605fede82ed2774d4933457e27473b1277a34f665aa`). Full returned results are saved in [evidence/size_constructor.json](evidence/size_constructor.json).

Recovered rules now used by `src/lua_api.lua`:

- Define **I(v)** as native integer conversion: preserve integers; parse complete numeric strings; floor floating inputs within signed64 range; failed conversion, NaN and infinity yield **0**. Store each dimension as **S(v) = signed32(I(v) mod 2^32)**. Thus **Size(-1.1,"2.9") = (-2,2)** and **Size(4294967297,2147483648) = (1,-2147483648)**. The constructor does not clamp negative dimensions.
- For a table, the first nonnil discriminator selects a complete pair: **width → width/height**, else **w → w/h**, else **[1]/[2]**. False and zero select their branch. Missing/invalid second fields become zero; no cross-family fallback. **Size{width=0,w=7,h=9} = (0,0)**; **Size{height=99,w="2.9",h="-1.1"} = (2,-2)**; **Size{height=99,h=98,4,5} = (4,5)**.
- Scalar arguments convert independently; omission yields zero. Native Size userdata is copied as **8 bytes**, containing two signed32 dimensions. The build returns an independent dimension table when copying its Size result.

`tests/native_size_constructor.rs` executes the real embedded Lua interface. Both regressions failed before correction: dimensions remained **-1.1,2.9**, and the short-field constructor returned **0,0** (`checks/native-size-red.log`). Both pass after correction. They cover signed wrap, negative fractions, numeric strings, invalid/nonfinite values, field precedence, false/nil and independent copies.

Verification: `cargo test --offline --locked` with writable isolated `XDG_DATA_HOME=/tmp/pixelcraft-size-audit/data` and `XDG_CONFIG_HOME=/tmp/pixelcraft-size-audit/config` passed **201 tests, zero failures, two ignored**, exit 0 (`checks/native-size-tests.log`). `cargo fmt --check` initially reported existing formatting differences in shortcut persistence code/tests; applied formatting and reran successfully. `cargo build --offline --locked` passed. Fresh Craft Anything `rebuild_check` passed build, desktop startup/drawing (**942×1005**) and mechanics validation (**40 values, 39 sourced, one estimated**). Inspected the resulting desktop screenshot.

Scope: this correction covers Size construction. Native userdata identity, operators and post-construction property setters remain outside this correction; the build continues to use Lua tables. Full application/extension equivalence remains unverified.

## Native audit — Point constructor coordinates (2026-11-11)

Freshly opened the original main ELF with `core_target_open` at `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`, with Deep mode enabled.

`native_strings_find` searched names, coordinate fields and messages with regex `^Point$|^x$|^y$|^grid_bounds$|invalid_fg_or_bg` (15 matches, `ev_94b30f5cd27998199a452aff2b21c30808d6608ed1bce4adea8074a598165e53`); a separate setting search `^color_bar$` returned `ev_e0b0dbb77a9b1088531be58043a76af03fc3d0cb234f7aac5290216a982ae7dc`. Settings/messages serve as navigation evidence only. Followed Point string `0xf409c0` to registration `0x3e481b` (`ev_faf74274dceb9e363c63be1828e77b0e0fe0b72e19ab0f5219a2c726f73e7c38`); coordinate string references `0x3e492c` / `0x3e4941` resolve the constructor helper entry `0x3e48f0` (`ev_80d8710f8caa70d2613309124e6706311224c77ca6a1eebff26abfba4b9d559c`). Followed wrapper/helper outgoing references to coordinate conversion (`ev_253dead6b7e93867b477eb323afdd4c14f67e330830118b2881357c6e471000d`, `ev_0df3f1d2b08adfaa0f4dc6c1a8a6217dd77d5f67ac44891fbf25afce2d009cec`).

Requested, awaited and read each `native_function_pseudocode` result separately, before requesting the next:

| Function | Address | Evidence |
| --- | --- | --- |
| `Point_new(lua_State*,int)` helper | `0x3e48f0` | `ev_4a7b57dcb2ad55d762e71446feb1021904ee30deaf17d54db76bdf09e9c348c6` |
| `lua_tointegerx` | `0x53eab0` | `ev_dd4c3f99a510b1ed5f1bf83f19090805b925fbd703b6729545ff1ad64994cad4` |
| `luaV_tointeger` | `0x554930` | `ev_1d294ffa1b28a6c31b9cc4b77d72801898b8fc48006492e4f491eb560c1de59b` |
| `Point_new(lua_State*)` wrapper | `0x3e4860` | `ev_7b5d2381271eedf71d35563313a4e0b07823001bb96d8f934d3845fb738d297b` |

Ghidra timed out with a temporary 15-second startup limit. All four completed reads use the built-in fallback decompiler and report `deep_engine_unavailable`; they are not successful Ghidra decompilations. The fallback approximates register arguments and flags. Full constructor assembly (`ev_5b860db18b5b04de3290348055e5c8ab538a4dff3965cf4529e3f334c7e396e7`) corroborates branch selection and 32-bit stores; fresh integer-helper assembly corroborates floor-mode handling. Restored the original null startup timeout. Full returned results are saved in [evidence/point_constructor.json](evidence/point_constructor.json).

Recovered rules now used by `src/lua_api.lua`:

- Define **I(v)** as Lua integer conversion: preserve an integer; parse a complete numeric string; floor a float when the result fits signed64; return **0** for invalid conversion, NaN or infinity. The native call passes mode **1** to the helper, which calls `floor`.
- Store each coordinate as **S(v) = signed32(I(v) mod 2^32)**. Thus **Point(-1.1,"2.9") = (-2,2)** and **Point(4294967297,2147483648) = (1,-2147483648)**. No byte saturation or truncation toward zero applies.
- For a table with **x nonnil**, convert its **x and y fields together**, even when x is false or invalid. Otherwise convert array entries **1 and 2 together**. No per-coordinate fallback: **Point{y=91,4,5} = (4,5)**; **Point{x=0,[1]=8,[2]=9} = (0,0)**; **Point{x=false,y=-1.1,9,8} = (0,-2)**.
- Missing scalar coordinates convert to **0**. The wrapper allocates **8 bytes** and copies two 32-bit coordinates; a native Point input copies those bytes. The build returns independent Lua coordinate tables, and construction copies their coordinates.

Regression `tests/native_point_constructor.rs` runs the real embedded Lua implementation and checks floor behavior, numeric strings, signed32 wrap, invalid values, nonfinite values, paired field selection and independent copies. Before correction it failed with coordinates **-1.1,2.9** instead of **-2,2** (`checks/native-point-red.log`); after correction it passes.

Verification: `cargo fmt --check` and `cargo build --offline --locked` passed. The initial full test run failed in `tests::native_assets_apply_inside_workshop_and_shortcuts_remain_local` because the sandbox made the normal preferences directory read-only (`checks/native-point-tests.log`). Reran the complete `cargo test --offline --locked` with `XDG_DATA_HOME=/tmp/pixelcraft-point-audit/data` and `XDG_CONFIG_HOME=/tmp/pixelcraft-point-audit/config`: **197 passed, zero failed, two ignored**, exit 0 (`checks/native-point-tests-isolated.log`). Craft Anything `rebuild_check` passed build, desktop startup/drawing (**942×1005**) and mechanics validation (**39 values, 38 sourced, one estimated**). Inspected the resulting desktop screenshot.

Scope: this audit corrects Point construction. Native userdata identity, Point operators and post-construction coordinate setters remain outside this correction; the build still uses Lua tables. Existing color, drawing and animation audits below retain their stated limits. Full original file/extension ecosystem compatibility remains unresolved.

## Native audit — HSV setter numeric conversion (2026-11-11)

Freshly opened the original main ELF with `core_target_open`: `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Deep mode was enabled.

Searched names, settings and messages using `native_strings_find` with regex `^hsvValue$|^hsvSaturation$|^hsvHue$|^color_bar$|invalid_fg_or_bg` (8 matches; `ev_e3749ca113de5ce80ac0b1eecd48e4ef85b91cb863508f029e01d6f07335a14d`). Followed hsvValue string `0xf3fbb2` to registration `0x1cbaed0` (`ev_116eefcc2d6b945622f1c7e576eddb5a1340900882444fd91ce468dcf7c9fa5a`). Exact setter-name references resolve function `0x3c6180` through pointer `0x1cbaee0`, 16 bytes after that registration (`ev_10950cc3898d0378ae49ff7828564dfae6561409bb84cd576ef465f0a73daf0c`). Its outgoing references lead to number conversion, HSV component access, alpha and construction (`ev_e7829f1621ea1a5b61240880916efc478be340f6b24ebbc0047fb2051da6e73d`). Settings/messages are navigation evidence, not newly inferred mechanics.

Requested, awaited and read each of these **eight completed pseudocode results separately**, before requesting the next:

| Function | Address | Evidence |
| --- | --- | --- |
| `_ZN3app6script12_GLOBAL__N_118Color_set_hsvValueEP9lua_State` | `0x3c6180` | `ev_99abdfaf811fefb1188011a0696b36a6eb50c073d6675968e520061ce17c4b90` |
| `_ZN3app6script12_GLOBAL__N_116Color_set_hsvHueEP9lua_State` | `0x3c5fa0` | `ev_5e3adf7b976571266cedae1c6347acd88d14992c4d493c69286b5da22042539c` |
| `_ZN3app6script12_GLOBAL__N_123Color_set_hsvSaturationEP9lua_State` | `0x3c6090` | `ev_4f70ea7eb7474a4dad1e4c2523a73d0bfd9168420a64da30cf16aa1f5290ae30` |
| `_Z13lua_tonumberxP9lua_StateiPi` | `0x53e9c0` | `ev_c2e604e25bb6aa2f2de721dece1e8aea2447549090270b5097823c72ce1eaffc` |
| `_Z14luaV_tonumber_PK6TValuePd` | `0x554760` | `ev_677df1b6d3d1b00712ec63d55d93e310ec5b7952c749c2d93e248441e975e959` |
| `_ZN3app5Color7fromHsvEdddi` | `0x16a5e0` | `ev_3982e68e9adb398e2856884b7231407f056c924eb1dc7f1a331f96c0a78d1da2` |
| `_ZN3app6script12_GLOBAL__N_113Color_set_hueEP9lua_State` | `0x3c6570` | `ev_b6a80dd05ad4d972ed8a661a64e396940e448268233d8505ff167ff2d32c6ecd` |
| `_ZN3app6script12_GLOBAL__N_120Color_set_saturationEP9lua_State` | `0x3c6700` | `ev_331aad626a44388b02aa612b6d4425587db3a8c7f36a90f606634e55a87e5bb7` |

Ghidra timed out with a temporary 15-second startup limit. These completed results came from the built-in fallback decompiler, whose register arguments, branch conditions and SIMD recovery are approximate; this is not a successful Ghidra analysis. Restored the original null startup timeout. Full returned pseudocode and limitations are retained in [evidence/hsv_setter_conversion.json](evidence/hsv_setter_conversion.json).

Recovered rules applied in `src/lua_api.lua`:

- Each explicit HSV setter calls `lua_tonumberx(L,2,null)` for the replacement component. Let **N(x)** be a double-valued numeric conversion: Lua integers/floats and valid numeric strings convert; nil, booleans, tables, functions and invalid strings produce **0.0**. `lua_tonumberx` initializes its result to zero and the failed conversion helper leaves it untouched. Neither truthiness nor direct Lua arithmetic implements this rule.
- Replacement calls are **fromHsv(N(x), oldS, oldV, oldAlpha)** for hue, **fromHsv(oldH, N(x), oldV, oldAlpha)** for saturation, and **fromHsv(oldH, oldS, N(x), oldAlpha)** for value. `fromHsv` writes representation **2**, three doubles at offsets **8/16/24**, and signed32 alpha at **32**; setters copy the complete result. Generic hue/saturation choose HSL for representation 3 and HSV otherwise. The value alias uses the same setter registration.
- The build now converts the incoming HSV setter value with **(tonumber(x) or 0)+0.0** before computing display RGB. Previously booleans/tables caused arithmetic errors, and string saturation/value could fail numeric comparison. Alpha and the existing representation-reset behavior are preserved.
- Examples from RGB green with alpha **73**: invalid hue gives red **(255,0,0,73)**; invalid saturation gives white **(255,255,255,73)**; invalid value gives black **(0,0,0,73)**. Assigning string hue **"120"** to red gives green; then string saturation **"0"** gives white; then string value **"0.5"** gives **(128,128,128,73)**.

Regression `tests/native_color_hsv_setter_conversion.rs` exercises actual embedded Lua Image pixels for all six property names, invalid input types, nil, numeric strings and preserved alpha. It failed before the correction with `attempt to perform arithmetic on a boolean value` (`checks/native-hsv-setter-red.log`) and passed after the correction.

Verification: fresh `cargo test --offline --locked` passed **180 tests, zero failures**, with one pre-existing ignored loopback transport test (`checks/native-hsv-setter-tests.log`). `cargo fmt --check` and `cargo build --offline --locked` passed. Craft Anything `rebuild_check` passed build, desktop startup/drawing (**942×1005**) and mechanics validation (**38 values, 37 sourced, one estimated**).

Scope: this pass corrects setter input conversion. The build still materializes HSV to RGB rather than retaining native HSV doubles/tag. Exact HSV conversion rounding, out-of-range/nonfinite inputs, HSV copy/equality and palette-dependent conversions remain compatibility gaps. Prior audits below retain their original scope and verification history.

## Native audit — HSV constructor selection and numeric inputs (2026-11-11)

Opened the original main executable using `core_target_open`, at `aseprite` (SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`). Deep mode was enabled. This is a fresh trace extending the earlier audits below.

Searched names, settings and messages with `native_strings_find`, regex `^Color$|^value$|^color_bar$|invalid_fg_or_bg` (`ev_370e8798d17225005cde4e33dff0915bdebc27425506a49f26b82b121dbca45e`). Followed `value` at `0xf78fd2` through its reference at `0x3c582e` (`ev_edec2a6655e7eb53ad40875018e02bcb3812964a817da96ed835d365e9ce3ad7`). `native_function_find` resolved this to `Color_new`, entry `0x3c54a0` (`ev_2821855b2d240cbcd24e86b60a37e6843b617948c5832aba8e78f101af28fac6`). Its outgoing references led to `fromHsv` and the numeric helpers (`ev_a7c03db519c1d0dcf73ee5146f3879f48cba4b007409a10277ed9ed67e6a576a`). Settings and message strings are navigation evidence, not numeric-rule evidence.

The first pseudocode request returned a transport timeout after 300 seconds, with no pseudocode; it is not counted as a completed read. Retried with a temporary 15-second Ghidra startup timeout, then requested, awaited and read each completed result separately before requesting the next:

| Function | Address | Completed pseudocode evidence |
| --- | --- | --- |
| `Color_new` | `0x3c54a0` | `ev_c08adf560f02e59509fdeb828fed87ba653040346e5ef2927c721671ec56a5cf` |
| `Color::fromHsv` | `0x16a5e0` | `ev_3982e68e9adb398e2856884b7231407f056c924eb1dc7f1a331f96c0a78d1da2` |
| `lua_tonumberx` | `0x53e9c0` | `ev_c2e604e25bb6aa2f2de721dece1e8aea2447549090270b5097823c72ce1eaffc` |
| `lua_tointegerx` | `0x53eab0` | `ev_dd4c3f99a510b1ed5f1bf83f19090805b925fbd703b6729545ff1ad64994cad4` |
| `luaV_tointeger` | `0x554930` | `ev_1d294ffa1b28a6c31b9cc4b77d72801898b8fc48006492e4f491eb560c1de59b` |

All five report `deep_engine_unavailable`: Ghidra timed out, and the completed results use the built-in fallback lifter. This is not successful Ghidra analysis. Its register arguments, flags and SIMD are incomplete; fresh full constructor assembly (`ev_757551102c7418e23bae1939870db61f9eaaf4e2b6e7ead130e320d086eec8fc`) corroborates the branch order and conversions. Previous integer-helper assembly below corroborates floor mode. Restored the original null startup timeout afterward. Full results and the initial timeout are retained in [evidence/color_hsv_constructor.json](evidence/color_hsv_constructor.json).

Recovered rules applied in `src/lua_api.lua`:

- Constructor selection uses the first **nonnil** discriminator in order **r, red, v, value, l, lightness, gray, index, tile**. In particular, HSV is selected by `v` or `value`, without requiring hue; HSV precedes HSL, gray and index. False and zero still select a branch. The build previously required hue and let gray/index override HSV.
- Short `v` selects **h/s/v/a only**; otherwise long `value` selects **hue/saturation/value/alpha only**. Missing or invalid H/S/V become **0.0** through `lua_tonumberx`; numeric strings are accepted. No fallback between short and long families occurs.
- Alpha is **255** if its selected field is nil, else **signed32(I(alpha))**. `I` preserves Lua integers, floors successfully converted numeric inputs into signed64, and returns zero on failed conversion. Construction does not saturate alpha: **-1.1 → -2**, **4294967369 → 73**. Packed RGBA masks its low byte, so alpha -2 packs as **254**.
- Native `fromHsv` stores representation **2**, unmodified H/S/V doubles at **+8/+16/+24**, signed32 alpha at **+32**. This pass corrects input selection/conversion; the build still materializes HSV into RGB and does not retain this exact native representation. Exact conversion rounding, out-of-range/nonfinite HSV, copying/equality of represented HSV and tile colors remain compatibility gaps.
- Examples: `Color{value=1}` draws **RGBA(255,255,255,255)**. `Color{hue=120,saturation=1,value=1,gray=73}` draws **RGBA(0,255,0,255)**. `Color{v=1,hue=120,saturation=1,alpha=73}` uses only short fields and draws **RGBA(255,255,255,255)**. `Color{v=false,value=1,h=120,s=1,a=false,gray=73}` draws **RGBA(0,0,0,0)**.

Both real embedded-Lua regressions in `tests/native_color_hsv_constructor.rs` failed before the correction (log `checks/native-hsv-constructor-red.log`) and passed afterward. They cover actual Image pixel output, value-only construction, precedence, field families, nil/false, numeric strings, failed conversion, alpha floor and signed32 narrowing. Fresh `cargo test --offline --locked` passed **177 tests, zero failures** (`checks/native-hsv-constructor-tests.log`); `cargo fmt --check` and `cargo build --offline --locked` passed. Fresh Craft Anything `rebuild_check` also passed build, desktop startup/drawing (**942×1005**) and mechanics validation (**37 values, 36 sourced, one estimated**). These checks establish this correction and a working build, not whole-application equivalence.

## Native audit — gray/index constructor precedence (2026-11-11)

Freshly opened the original main ELF with `core_target_open`: `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`, in Deep mode. This pass extends the prior native audits below.

`native_strings_find` searched names, settings and messages with regex `^Color$|^value$|^gray$|^color_bar$|invalid_fg_or_bg` (10 matches; `ev_dc160a3ccb61bed55b3869723e679afb77c4f9a5f07cf12c56ad315def6a2da8`). Followed gray at `0xf600c9` through its reference at `0x3c5a28` (`ev_9cd28cd5458e0bcbfae1d4bf1cbecfff9acc7d2449547224a9139fce00b8138a`); `native_function_find` resolves `Color_new` at `0x3c54a0`. Followed outgoing references (`ev_73e1c01716d1f56fddee04251ac0d7ca174f0b3bb4e103e9b83ba8324f75301e`) to gray construction and numeric conversion. Settings/messages were navigation anchors, with no new behavior claims.

Requested, awaited and read each of these four completed pseudocode results separately before requesting the next:

| Function | Address | Evidence |
| --- | --- | --- |
| Color_new | `0x3c54a0` | `ev_8f616ea4abbe1759c2172730e27f3036d738b1f691cc83072402211fe9eac114` |
| Color::fromGray | `0x16a620` | `ev_23173653adbf639903f24c8f431c96e617d285ae39aa7624162524b28c409cbd` |
| lua_tonumberx | `0x53e9c0` | `ev_8484bb763372a6ad99788d4cfd2e9406e85ea72125b6539a395e2f0a35181201` |
| lua_tointegerx | `0x53eab0` | `ev_446b3298f424da3a883b910c3f9d2b4e03b46a78a938a495ee135510f2db4692` |

Ghidra timed out with a temporary 15-second startup limit. These are completed built-in fallback decompiler reads, with register-level and approximate-condition limitations; they are not successful Ghidra decompilation. Restored the original null timeout afterward. Full results: [evidence/color_constructor_priority.json](evidence/color_constructor_priority.json). Fresh constructor assembly (`ev_e4ec748b33d27705fb9154559dc5bf2eb563960013d34b0e538c8d48a7e57bdf`) is saved in [evidence/color_constructor_priority_assembly.json](evidence/color_constructor_priority_assembly.json).

Recovered rules applied in `src/lua_api.lua`:

- At `0x3c5a35..0x3c5a3c`, nonnil **gray** selects gray construction. Only nil gray reaches the **index** test at `0x3c5a82`. False, zero and invalid numeric strings still select gray. The build previously tested index first; this pass corrects that order.
- Gray uses **G=trunc_signed32(number(gray))**; failed number conversion is zero. Alpha uses the long **alpha** field only, defaults to **255** when nil, otherwise **A=signed32(I(alpha))**, where the previously audited integer helper floors successful numeric conversions and returns zero on failure. No channel saturation occurs during construction. `fromGray` stores representation **4**, signed32 gray at **+8** and signed32 alpha at **+12**.
- Example: `Color{gray=73.9,index=0,alpha=42.9}` retains gray **73** and alpha **42**; its packed pixel is **RGBA(73,73,73,42)**. `Color{gray=false,index=0,alpha=-1.1,a=99}` retains gray **0**, alpha **-2**, and gray representation. Short `a` is ignored. Existing RGB/HSL precedence is retained.

Both embedded-Lua regressions in `tests/native_color_constructor_priority.rs` failed before the correction and passed afterward. They cover actual Image pixel output, retained representation/copying, nonnil false and invalid strings, truncation versus alpha floor, alpha field family and higher-priority RGB/HSL forms. Failure log: `checks/native-constructor-priority-red.log`.

Verification: `cargo test --offline --locked` passed **152 tests, zero failures**. `cargo fmt --check` and `cargo build --offline --locked` passed. Fresh Craft Anything `rebuild_check` passed build, desktop startup/drawing (**942×1005**) and mechanics validation (**36 values, 35 sourced, one estimated**).

Scope: this correction covers gray versus index selection. Exact HSV representation/conversion, HSV mixed-table selection and tile colors remain compatibility gaps; no full native-parity claim is made.

## Native audit — HSL table construction (2026-11-11)

Freshly opened the original main ELF with `core_target_open`: `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Deep mode was enabled. Earlier audits below remain applicable; this pass closes the HSL table-constructor gap explicitly listed by the previous audit.

Searched names, settings and messages using `native_strings_find`, regex `^Color$|^lightness$|^color_bar$|invalid_fg_or_bg` (`ev_873ef2dcfa520ce47c20921f4c10dfaea858ad5281c56ee2292e37df99693111`). The `lightness` string at `0xf3a294` references `0x3c5953`; `native_function_find` resolves it to `Color_new` at `0x3c54a0` (`ev_cea15fef5dee6196f6f49d467bbe11863439522c2fae493f8342b985ced24f6d`). Followed its outgoing references to `fromHsl`, `lua_tonumberx` and `lua_tointegerx` (`ev_73e1c01716d1f56fddee04251ac0d7ca174f0b3bb4e103e9b83ba8324f75301e`). The settings/message matches are navigation anchors, with no new behavior claim.

Requested, awaited and read these **four completed pseudocode results sequentially**, one before requesting the next:

| Function | Address | Evidence |
| --- | --- | --- |
| Color_new | `0x3c54a0` | `ev_8f616ea4abbe1759c2172730e27f3036d738b1f691cc83072402211fe9eac114` |
| Color::fromHsl | `0x16a600` | `ev_63b0b702fc5ac666b087813424ede543e30584ff1cb41d725f25c046a5bc8a71` |
| lua_tonumberx | `0x53e9c0` | `ev_8484bb763372a6ad99788d4cfd2e9406e85ea72125b6539a395e2f0a35181201` |
| lua_tointegerx | `0x53eab0` | `ev_446b3298f424da3a883b910c3f9d2b4e03b46a78a938a495ee135510f2db4692` |

Ghidra timed out with a temporary 15-second startup limit; the tools returned completed built-in fallback pseudocode, with approximate register-level conditions/arguments and inline assembly. Restored the startup limit to its original null value after the reads. Full results, outgoing references and constructor assembly (`ev_e4ec748b33d27705fb9154559dc5bf2eb563960013d34b0e538c8d48a7e57bdf`) are retained in [evidence/color_hsl_constructor.json](evidence/color_hsl_constructor.json). Assembly corroborates the field tests, argument order, alpha conversion and complete 40-byte result copy.

Recovered rules applied in `src/lua_api.lua`:

- Constructor table selection tests **presence (not nil)** in order: **r, red, v, value, l, lightness, gray, index, tile**. This pass corrects HSL selection and the existing HSV/HSL conflict; broader HSV/gray/index mixed-table priority remains unresolved.
- With short `l` present, HSL uses **h, s, l, a** only; otherwise long `lightness` selects **hue, saturation, lightness, alpha** only. Hue is not required. At `0x3c5998..0x3c5a11`, H/S/L each use `lua_tonumberx`; missing or failed conversions become **0.0**, numeric strings are accepted. Alpha defaults to **255** only when its selected field is nil; otherwise it uses **signed32(I(alpha))**, where the original integer helper uses floor conversion and returns zero on failure. Alpha is not saturated by construction. Thus -1.1 stores **-2**, while 4294967369 stores **73**.
- `fromHsl` stores tag **3**, unmodified double H/S/L at **+8/+16/+24**, and signed32 alpha at **+32**. The constructor retains this representation, enabling the previously recovered HSL getters, copying, tolerance equality and generic hue/saturation edits. Display RGB follows the existing native-derived HSL formula below; gray remains **signed32-trunc(L*255)** from stored lightness.
- For example, `Color{lightness=0.5}` has H/S/L **0/0/0.5**, RGB **128/128/128**, gray **127**, alpha **255**. `Color{h=120,s=1,l=0.5,a=73}` draws opaque-channel green with alpha **73**. `Color{l=0.5,hue=120,saturation=1,alpha=73}` ignores the long fields and yields gray with alpha **255**. Stored lightness **2** remains **2**, displays white and gives gray **510**.

Two real embedded-Lua regressions in `tests/native_color_hsl_constructor.rs` failed before the correction (lightness-only selection and field-family mismatch). They cover image pixel output, component retention/copy/edit, short/long families, nonnil false fields, numeric strings, failed conversion, signed alpha, RGB/HSV precedence and HSL precedence over gray/index. Logs: `checks/native-hsl-constructor-{red,green,tests,build}.log`.

Verification: `cargo test --offline --locked` passed **148 tests, zero failures**. `cargo fmt --check` and `cargo build --offline --locked` passed. Craft Anything `rebuild_check` passed build, desktop startup/screenshot (**942×1005**) and mechanics validation (**35 values, 34 sourced, one estimated**).

Scope: this pass corrects HSL table construction using the existing native-derived HSL conversion. Exact HSV representation, wider mixed-constructor priority, palette-dependent conversions and exhaustive NaN/infinity behavior remain unresolved. Completed fallback pseudocode plus assembly is not a successful Ghidra analysis or a claim of full native parity.

## Native audit — represented HSL color mutation (2026-11-11)

Opened the original main ELF with `core_target_open`: `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Deep mode was enabled. This audit extends the earlier drawing, animation, blending, preferences and scripting audits preserved below.

Searched names, settings and messages with `native_strings_find`, regex `^Color$|hslLightness|hslSaturation|hslHue|^color_bar$|invalid_fg_or_bg` (12 matches; `ev_e27b74fddc79e987c0a9192737ef64ee9c67d6714d62ad80a600446119ed132d`). Followed `hslLightness` at `0xf3fbd0` to property entry `0x1cbaf18` (`ev_996c311407176a157b847b9a2c74b5de546f6dafcd11ab7daf12ef6ed9efec3c`). Native-name references identify its setter `0x3c6450` via pointer `0x1cbaf28` (`ev_4f4e4901d2911e722b7d7be26a895ab74581d3d5ccc03c7fad68909a9976fc85`). Outgoing references lead to HSL getters, alpha, number conversion and `fromHsl` (`ev_5a0624ff9b125f8e233486c2dd6f09042ac8a46f55c186f909814f9e03cf2a6c`). Settings/messages are navigation anchors, not new behavior claims.

The first pseudocode request timed out after 300 seconds and is **not** counted as a completed read. Retried with a temporary 15-second Ghidra startup timeout; Ghidra still failed, but the tool returned completed built-in fallback pseudocode. Read each of the following **16 completed results sequentially**, awaiting and reading each before requesting the next. Restored the startup timeout to its original null value. Full results, with their register/condition/inline-assembly limitations, are saved in [evidence/color_hsl_mutation.json](evidence/color_hsl_mutation.json).

| Function | Address | Pseudocode evidence |
| --- | --- | --- |
| Color_set_hslLightness | `0x3c6450` | `ev_19cd812e60ae85a17bcea8719b7227c967335172b5a82a0ba64d83392c643ec1` |
| Color::fromHsl | `0x16a600` | `ev_63b0b702fc5ac666b087813424ede543e30584ff1cb41d725f25c046a5bc8a71` |
| Color_set_hslHue | `0x3c6270` | `ev_af265ad2695ffa861e3aecd1cfcc8e10baaf35436490f6d6204ba89200ce16ea` |
| Color_set_hslSaturation | `0x3c6360` | `ev_6bab2713f7f22e3da45e039407a2581f6265c3b75b92ac57a62b05bb6fea80d7` |
| gfx::Hsl(H,S,L) | `0x691370` | `ev_0a1ee0c3a38770329159d0e5280eef52110f2b23315e8ae820f7ffd50fc9fb94` |
| gfx::Rgb(Hsl) | `0x6918e0` | `ev_d46c19fe74415f9b9a385b164a4652e62fb257c72bc098406407cd29168d3050` |
| Color::getHslHue | `0x16c860` | `ev_cd6767c1679a0b43b3d0c7dd8a5094a10109ba265dbcc884991e0bf588460dd9` |
| Color::getHslSaturation | `0x16c960` | `ev_ffae17c2a857648fee86a09bec7498062fd2fbbac921333d7b6090bae83fc278` |
| Color::getHslLightness | `0x16ca60` | `ev_8cd4fee6e6ebd1e4a8b93073ae00eba154bfe6818238638b1a4982f846c4eabe` |
| gfx::Hsl(Rgb) | `0x6913b0` | `ev_d5859753671dc68077a7bf79376b87502d888c39f20fa2afcccbc62137e0ac72` |
| Color::getGray | `0x16c2b0` | `ev_7533296ec8ca2301e66c93dff920987883d6536509c257712f2c659199ffb0bb` |
| Color::operator== | `0x16c3f0` | `ev_122d02c2349f0eaaa1ab992fb12a1966cba4500a25984ac984c92f389902c905` |
| Color::getRed | `0x16ae40` | `ev_04da219a05d111d1cee2c0a5279449e611b399fa723e0939c329a927d9d04699` |
| Color::setAlpha | `0x16cdc0` | `ev_a1e14441479a48dc0ac957324987a491f6526ad4e9c56ba514977be1341e0ee9` |
| Color_set_hue | `0x3c6570` | `ev_eb8a7684c45af9a935a93a7d209080413462e7a595eb62ac04e541a4759c402e` |
| Color_set_saturation | `0x3c6700` | `ev_af99d47701d0b85d5a6c3aac27a197919633276dbadcfee12d8f435445f1d93a` |

Assembly corroboration and raw ELF data are retained in [evidence/hsl-rgb-assembly.txt](evidence/hsl-rgb-assembly.txt) and [evidence/hsl-data-tables.json](evidence/hsl-data-tables.json). These use ELF load-segment address mapping rather than file offsets as virtual addresses. Tag tables resolve the fallback's indirect branches. Comparator assembly is `ev_2ec9953a1f6e7219cb8c8358333eeee5cf577457c5bcb12b559f618303638449`.

Recovered rules now applied in `src/lua_api.lua`:

- **Representation:** `fromHsl` stores tag **3**, double H/S/L at **+8/+16/+24**, and signed32 alpha at **+32**. It does not clamp the stored doubles. Each explicit HSL setter replaces the complete **40-byte** color with `fromHsl(number(new),S,L,A)`, `fromHsl(H,number(new),L,A)`, or `fromHsl(H,S,number(new),A)`. The changed component uses `lua_tonumberx`; failed numeric conversion gives zero. Replacing gray/index discards that representation; copying retains an independent HSL tuple.
- **Getters:** HSL getters return stored doubles directly for tag 3. Gray-to-HSL uses **H=0, S=0, L=gray/255.0**, including the full signed gray value. For RGB, let max/min be signed channels: **D=signed32(max-min)/255.0**, **L=(signed32(max+min)/255.0)*0.5**. Achromatic H/S are zero; otherwise S is **D/(1-abs(2L-1))**, and hue uses the maximum-channel branch, normalized red branch modulo 6, then multiplies by **60**.
- **RGB materialization:** `gfx::Hsl(H,S,L)` clamps S and L to **[0,1]**, retaining hue. `gfx::Rgb(Hsl)` computes **C=(1-abs(2L-1))*S**, **h=H/60**, **X=C*(1-abs(fmod(h,2)-1))**, **m=L-C/2**. Signed truncation of h selects sectors **0..6** with tuples **(C,X,0), (X,C,0), (0,C,X), (0,X,C), (X,0,C), (C,0,X), (C,X,0)**. Other sectors use (0,0,0). Channels are **signed32-trunc((component+m)*255+0.5)**; hue is not unconditionally wrapped modulo 360.
- **Gray:** for represented HSL, **gray=signed32-trunc(stored L*255)**, using stored lightness rather than rounded display RGB. Thus L=0.5 gives gray **127** while RGB is **128,128,128**. L=2 remains stored as 2, displays white and gives gray **510**. The existing low-byte grayPixel packing is retained.
- **Equality:** representations must match. HSL equality requires **abs(H1-H2)<0.001**, **abs(S1-S2)<0.00001**, **abs(L1-L2)<0.00001**, and exact alpha equality. Constants at `0xf7b400` and `0xf7b408` were read directly (`ev_47584876770b5650099d9d0ce39ca70e8f721050419bbe1b8742e45c558b3b26`, `ev_596af021f987cb4272aef163e7dad9c39b7c5011046983b7e1294446cfaf1524`). This is not packed-pixel or exact-double equality.
- **Aliases and replacement:** generic hue/saturation setters select HSL only when the current tag is 3; otherwise they select HSV. Lightness uses HSL. Alpha edits preserve HSL state; RGB, gray, index and explicit HSV assignments replace it.

The previous implementation silently stored HSL assignments as ordinary Lua fields. The corrected implementation changes the actual channels and retains the represented doubles, including achromatic hue and unrounded saturation. Six embedded-Lua regressions in `tests/native_color_hsl_mutation.rs` cover visible edits, fractional retention, independent copying, replacement, alpha, gray conversion, equality tolerances, aliases, signed32 gray transitions, sector 6 and strict equality boundaries. Fail-before logs are `checks/native-hsl-{red,red-expanded,alias-red,gray-red,sector-red}.log`.

Verification: `cargo test --offline --locked` passed **144 tests, zero failures** (`checks/native-hsl-tests.log`). `cargo fmt --check` and `cargo build --offline --locked` passed (`checks/native-hsl-build.log`). Craft Anything `rebuild_check` passed build, desktop startup/screenshot (**942×1005**) and mechanics validation (**34 values, 33 sourced, one estimated**). Review found and corrected the signed-gray transition and sector-6 cases; both have fail-before/pass-after regressions.

Scope: this audit corrects HSL **mutation** and copying of the resulting colors. Existing HSL/HSV table constructors and exact HSV representation retention remain separate compatibility gaps. Cross-model HSV conversion and palette-dependent getters retain existing limitations. NaN/infinity and every floating boundary have not been exhaustively compared against the running original. Completed fallback pseudocode plus assembly/data corroboration is not a claim that Ghidra succeeded or that the whole application has native parity.

## Current native audit — HSV edits discard gray state (2026-11-04)

Freshly opened the original main ELF using `core_target_open`: `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Deep mode was enabled.

Searched names, settings and messages using `native_strings_find` with `^alpha$|^Color$|^color_bar$|invalid_fg_or_bg` (`ev_814cd54cdbf79222045a8d3a6eba99cf3c6e3d72811f511f82f17c47c3f68434`). Followed Color string `0xf446eb` to registration `0x3c5320` (`ev_49dfe1a13a241e13cfbbe24dc8a759f7a9e42c3cc9d4a8596b4dd01e95ba9adb`), then its property table `0x1cbae40` (`ev_9346ec2e59fb509f7cd9dbc4a924dbad69c062533c39acbf15bbe4d304c61e5c`). Exact native-name references identify `Color_set_hsvHue` at `0x3c5fa0` from table entry `0x1cbaeb0` (`ev_d41b095283f4241a1b3d2736dbcd149a1318d5f176966c3de17b1350a586881c`). Outgoing references lead to `fromHsv` at `0x16a5e0` (`ev_57a910379427809d7b9d9ae6389b536fe78693074b351b3fc26b94781c213103`). Settings/message strings are navigation anchors; this audit adds no claims about their behavior.

Read every completed `native_function_pseudocode` before requesting the next. The five HSV/gray results are preserved in `evidence/color_hsv_mutation.json`:

| Function | Address | Evidence |
| --- | --- | --- |
| Color_set_hsvHue | `0x3c5fa0` | `ev_0d5c4be8add50d3f626bf98c223b5b09e1d2c75c70a04bc623a21929affc4a29` |
| Color::fromHsv | `0x16a5e0` | `ev_91335ad2a58d678b5806cf881c518f35fc7b7ea6391c5b9a40b791a044c18e20` |
| Color_set_hsvValue | `0x3c6180` | `ev_59eb5799538207f297e96a8ef808d057d2a3da572e884638c5228565c1b5e4e2` |
| Color_set_hsvSaturation | `0x3c6090` | `ev_18f8534638575b79358b430bd09055ec0d28344a8f18c1ea8b335dbe96d931a1` |
| Color::getGray | `0x16c2b0` | `ev_7533296ec8ca2301e66c93dff920987883d6536509c257712f2c659199ffb0bb` |

Also read alpha wrapper `0x3c5f30` and `setAlpha` `0x16cdc0` (`ev_21bd25b5bf87a47cb1522080f17388624df20ff67b9aab4c3cd569e43f1c582f`, `ev_a1e14441479a48dc0ac957324987a491f6526ad4e9c56ba514977be1341e0ee9`). No alpha correction was needed.

Ghidra timed out; these are completed built-in fallback results, with approximate register types, arguments and conditions. The temporary 15-second Ghidra startup limit was restored to null. Setter assembly (`ev_d21689ff6f5b52b72ab7b18b44b081871fb911cc71bbe3dc0ea437e3eede9759`) and factory assembly (`ev_411c06400636413ebfb32a9d5065850c091c583281a0df0f07273593d925ae18`) corroborate the replacement independently of the fallback's argument reconstruction.

Recovered rules:

- Let `(H,S,V,A)` be the current HSV getters and alpha. Explicit HSV setters replace the value with `fromHsv(number(v),S,V,A)`, `fromHsv(H,number(v),V,A)`, or `fromHsv(H,S,number(v),A)`. They use `lua_tonumberx`, not byte/integer conversion, for the changed component.
- `fromHsv` writes representation tag **2**, doubles H/S/V at offsets **+8/+16/+24**, and signed32 alpha at **+32**. It performs no clamp or RGB conversion. Setter `0x3c6180` copies the entire **40-byte** result back at `0x3c6201..0x3c6218`. Thus a prior gray/index payload is replaced, and alpha is preserved.
- Gray getter `0x16c2b0` dispatches on the current representation. It cannot keep returning a former gray payload after an HSV replacement. In the tested achromatic case, black gray **0**, alpha **73**, then HSV value **1** becomes white: gray **255**, grayPixel **18943 = 255 + 73*256**. Changing value back to **0** gives grayPixel **18688 = 73*256**.

Applied correction: `src/lua_api.lua` now clears old gray state as well as index state in its existing HSV mutation path. `tests/native_color_hsv_mutation.rs` invokes the actual embedded Lua API and checks updated channels, gray/grayPixel, preserved alpha, and independent copies. It failed before correction with `HSV edit must discard the old gray payload`.

Verification: `cargo test --offline --locked` passed **136 tests, zero failures** (`checks/native-hsv-tests.log`). `cargo fmt --check` and `cargo build --offline --locked` passed (`checks/native-hsv-build.log`). `rebuild_check` passed build, desktop startup/screenshot (**942×1005**) and mechanics validation (**31 values, 30 sourced, one estimated**).

Scope: this fixes stale gray state in the existing RGB-backed HSV implementation. Exact native double HSV retention, out-of-range component behavior, HSV/HSL representation equality, and conversion rounding remain compatibility gaps; the RGB-backed conversion is a project approximation, not the native factory rule. Prior verified mechanics and limits remain below.

## Current native audit — represented Color equality (2026-11-04)

Opened the original main executable with `core_target_open` at `aseprite`. The previous installed path no longer exists. SHA-256 is the identical original: `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`.

`native_strings_find` searched names, settings and messages using `^Color$|^__eq$|^color_bar$|invalid_fg_or_bg` (`ev_64486c6ce3c6aac1de40598a94de7c2daed2e8f34cb44c991891d5029b4b39bf`). Followed Color string `0xf446eb` to registration `0x3c5320` (`ev_49dfe1a13a241e13cfbbe24dc8a759f7a9e42c3cc9d4a8596b4dd01e95ba9adb`), then its methods table `0x1cbae10` (`ev_9346ec2e59fb509f7cd9dbc4a924dbad69c062533c39acbf15bbe4d304c61e5c`). The table entry at `0x1cbae20` pairs the `__eq` string pointer `0xf3fb98` with wrapper `0x3c5c10` (`ev_f8037a234bbaa550168997547bde067dd26b15b5f4c7968005e1d8dffbff8b09`). Outgoing wrapper references call `Color::operator==` at `0x16c3f0` (`ev_ae7b58e27e13f2f8914b359fb696b4ef033f0e8d9abe81338df90850d4890971`). Settings/messages are navigation anchors, with no new claims about their behavior.

Completed and read five `native_function_pseudocode` results in this order, awaiting every result before requesting the next:

| Function | Address | Evidence |
| --- | --- | --- |
| Lua Color_eq | `0x3c5c10` | `ev_45aa5407c15e7c908733f4dbd17d075e8a27a659dc3efe86918c61ca0be743fd` |
| Color::operator== | `0x16c3f0` | `ev_122d02c2349f0eaaa1ab992fb12a1966cba4500a25984ac984c92f389902c905` |
| Color::fromRgb | `0x16a5c0` | `ev_58c3cf8768e5fef1bb1353661d7987154e29b58dd19d7110513367d15f99b2c8` |
| Color::fromGray | `0x16a620` | `ev_23173653adbf639903f24c8f431c96e617d285ae39aa7624162524b28c409cbd` |
| Color::fromIndex | `0x16a630` | `ev_a17492d37e8fdfa3887d6c4c20ecee6eff2fc85874b7e88ddda038fe03f06b83` |

Ghidra timed out; all five results use the built-in fallback with approximate register types and conditions. A temporary 15-second startup timeout was restored to its original null value. Full returned pseudocode and comparator assembly are saved in `evidence/color_equality.json`. Assembly (`ev_2ec9953a1f6e7219cb8c8358333eeee5cf577457c5bcb12b559f618303638449`) resolves the fallback's missing `cmp` operations before `sete`. Jump table bytes at `0xf81a20` (`ev_7c5b3b3ef5155c0ccf211c8d2f6a1fad6b817c22cd188a030b5b72daad72835a`) resolve tag branches.

Recovered rules now used by `src/lua_api.lua`:

- Let `T` be the representation tag. If `Tx != Ty`, equality is false (`0x16c3f0..0x16c3f4`). RGB uses tag **1**, gray **4**, index **5**, as stored by the factory functions above.
- **RGB:** equality is `Rx=Ry && Gx=Gy && Bx=By && Ax=Ay`, comparing all four stored 32-bit integers at offsets **+8,+12,+16,+20** (`0x16c48c..0x16c4ad`). There is no byte wrapping before comparison. Thus red **1** differs from **257**, even though both pack to byte **1**.
- **Gray:** equality is `Gx=Gy && Ax=Ay`, comparing stored 32-bit integers at **+8,+12** (`0x16c477..0x16c488`). Gray **5** differs from gray **261**, alpha **255** differs from **511**, and gray differs from RGB with identical visible channels.
- **Index:** equality is `Ix=Iy`, comparing stored signed32 index at **+8** (`0x16c4b1..0x16c4ba`). It does not compare palette RGBA. Invalid indices **-1** and **-2** therefore differ even if the build's fallback pixels coincide.
- Equality uses current private representation state, so copying preserves equality and gray/index assignment changes the comparison branch.

`tests/native_color_equality.rs` runs the actual embedded Lua API. Both tests failed before correction (wrapped RGB equality and gray/RGB equivalence), then verify stored channels, alpha, gray/index distinctions, independent copies and representation replacement.

Verification: `cargo test --offline --locked` passed **132 tests, zero failures** (`checks/native-equality-tests.log`). `cargo fmt --check` and `cargo build --offline --locked` passed (`checks/native-equality-build.log`). Craft Anything `rebuild_check` passed build, desktop startup/screenshot (**942×1005**) and mechanics validation (**30 values, 29 sourced, one estimated**).

Scope: this correction covers the existing RGB, gray and indexed representations. Native tags **2/3** use floating payload comparisons, and tag **0** has a mask branch; this build does not yet retain those representations. Full HSV/HSL/mask equality, palette-dependent getters and original-application parity remain unresolved. Earlier audit results below are retained as historical verification.

## Current native audit — indexed Color input conversion (2026-11-02)

Opened the original main ELF with `core_target_open`: `aseprite`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Earlier drawing, timing, blending, settings and scripting audits remain below.

Searched names, settings and messages with `native_strings_find`, regex `^Color$|^index$|^color_bar$|invalid_fg_or_bg` (nine matches; `ev_ad4b3f6ffc33a22b7218aec90988fa279ce281051c9010485d3e0ece7b3c20fc`). Followed index string `0xf60657` to constructor reference `0x3c5a75` (`ev_a3c9493d3421912cd43a7745fcd3c6df5a311b9d7030d0d7f303e0efd0b5bd2e`), then outgoing calls to `lua_tointegerx` and `fromIndex` (`ev_987616d99e92a4c7decfef8462048c61c865fe1f79087deac94622bf77caa942`). Function search located index getter/setter; their outgoing references identify the same factory and the native getter (`ev_0a3051eabc89235885badfc3ba6b188c3a31676984682f07e6e3afb5e74bed59`, `ev_07ac34e5166ff0dc10ab4e050dac6c2547fb2d0a613d760a06cc5125014ca4f9`). Settings/message matches are navigation anchors; this audit makes no additional claims about them.

Read seven completed `native_function_pseudocode` results sequentially, awaiting each before requesting the next. Full results: `evidence/color_index_pseudocode.json`.

| Function | Address | Pseudocode evidence |
| --- | --- | --- |
| Color_new | `0x3c54a0` | `ev_e3391ea5d6f591f7440051acc25ba72878947603d4cfcf7370cb554470dcd309` |
| Color::fromIndex | `0x16a630` | `ev_48c98fc7f29d194d139ece99500d684998481fe2a53eebbcef1f16b246e06eaa` |
| lua_tointegerx | `0x53eab0` | `ev_78c9a310d50d382a08c8075b32d95dfb2ca81997767d81b8e819b090e8d6f085` |
| luaV_tointeger | `0x554930` | `ev_7dd36e27cd0eb6dae6de9ba81c4d37681caf8ae8b8398562a3f91b06bf401f51` |
| Color_set_index | `0x3c6860` | `ev_2a171f406d3497d497ce01e70a613edf26d4d463ca9385e746cd513937a7c295` |
| Color_get_index | `0x3c6830` | `ev_4cbe9b0b6e3f883eff0fc1d7f63fbd1f29cfb074c025491ab8e87a130cbfce1e` |
| Color::getIndex | `0x16cbb0` | `ev_19e5bcf1f5e63d9de4662eaf3a18d325718870958a7dc1aee943806eb763a888` |

The first constructor call timed out at the tool's 300-second limit; it produced no usable pseudocode. Retried with a temporary 15-second Ghidra startup limit, then restored the original null setting. All seven completed results explicitly report Ghidra unavailable and use the built-in fallback. Its types, conditions and register arguments are approximate. Supplemented with full setter/getter assembly (`ev_37dacadc973f7b0055eacac58c0fea8b2ae7d034a12bb106d4e03da8ffd92242`, `ev_89a8674139f3bba0b63b138b437edeadf4c7e89c3080856ab0043f2ced719aa6`, `ev_28494725682008cd3f8d7d0e211e75f2ee467782c175990df211d8b6a1bae556`; saved in `evidence/color_index.asm` and ledger). Constructor and numeric-helper assembly from the earlier audits is retained in `evidence/color_constructor.asm` and `evidence/luaV_tointeger.asm`.

Recovered rules applied to `src/lua_api.lua`:

- Both explicit index table construction (`0x3c5a95..0x3c5aa1`) and index assignment (`0x3c6896..0x3c68a2`) convert input through `lua_tointegerx`, then pass its low 32 bits to `fromIndex`.
- Define **I(v)** as integer identity for signed64 Lua integers; otherwise floor a convertible finite number/numeric string within **[-2^63,2^63)**; failed conversion returns **0**. Define **U=I(v)&0xffffffff**, then **N=U** if U<2^31, otherwise **N=U-2^32**. No palette-range or byte saturation occurs in index storage. Examples: **'2.9'→2**, **-1.2→-2**, **false→0**, **NaN→0**, **0x100000003→3**, **2147483648→-2147483648**.
- `fromIndex` stores representation tag **5** and signed32 N at offset **+8**. Assignment replaces the prior represented value, so stale gray state must be discarded.
- Indexed `getIndex` returns stored N. Verified jump table slot **5** at **0xf81adc** contains signed offset **-14765729** from table base **0xf81ac8**, landing at **0x16cc27**, which loads offset +8 (`ev_992edcc11f9022ea8bfc6ba93a9d4357a0377efcf327bc0f8e5ae1511996b657`). Lua wrapper sign-extends EAX at **0x3c684d** before pushing the integer.

Previously the build retained input strings/fractions/booleans/wide integers and assignment also left old gray state. Both regressions in `tests/native_index_conversion.rs` failed before their corresponding corrections. They run the real embedded Lua interpreter and verify actual painted document pixels.

Final verification: `cargo test --offline --locked` passed **114 tests, zero failures** (`checks/native-index-tests.log`); `cargo fmt --check` and `cargo build --offline --locked` passed (`checks/native-index-build.log`). Craft Anything `rebuild_check` passed build, desktop startup/screenshot (938×990), and mechanics validation (**29 values, 28 sourced, one estimated**).

Scope: corrected explicit index input normalization and represented-state replacement. Palette lookup remains a snapshot in this build; native palette changes, invalid-index RGBA/alpha behavior, nearest-color matching for non-indexed colors, mixed constructor precedence and full HSV/HSL parity remain unresolved. Full original-application parity is not claimed.

## Current native audit — Color value copying (2026-11-02)

Freshly opened the original main executable `aseprite` with `core_target_open` in Deep mode. SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Earlier audits of drawing, timing, settings, blending and scripting remain below.

Searched names, settings and messages with `native_strings_find`, regex `^Color$|^gray$|^color_bar$|invalid_fg_or_bg`; all nine matches returned (`ev_af44c136f58203fe29b8847d0179fe62571c33cb884f1d091f0cb4c4a18df017`). Followed gray string `0xf600c9` to `Color_new` reference `0x3c5a28` (`ev_02660b82be029b8e3ce6b15290cdb340313a93ad0c04777e7568c8114acc8865`) and its outgoing calls to `fromGray` (`ev_987616d99e92a4c7decfef8462048c61c865fe1f79087deac94622bf77caa942`). Setting and message matches are navigation anchors; no new setting or alert behavior is claimed here.

Completed and read these four `native_function_pseudocode` results sequentially, awaiting each result before the next function request. An initial constructor request completed without its payload being displayed by the wrapper; it was repeated and its full result read before continuing. Results are retained in `evidence/color_copy_pseudocode.json`.

| Function | Address | Completed pseudocode evidence |
| --- | --- | --- |
| Color_new | `0x3c54a0` | `ev_e3391ea5d6f591f7440051acc25ba72878947603d4cfcf7370cb554470dcd309` |
| Color::fromGray | `0x16a620` | `ev_46b28f301ef06ad6f24ad99a51d0496f5101113c40ae3400056889c819a7e16b` |
| Color::getGray | `0x16c2b0` | `ev_cb4b93a186f1ca43bf524dca5698f19da1edac3405b8c0dbc13919067ec6c73c` |
| Color::getAlpha | `0x16b2a0` | `ev_410e7153101ea7480fb0a868ee24d4c43cca5c01a933bb6cbe70db57dab4facb` |

Ghidra timed out. These are completed built-in fallback decompilations with approximate register types and conditions. The constructor assembly was also retrieved (449 instructions, `ev_e15bd624ea78eb0761226710ff962f1b10fe28ec27bdbc385597543d6746b2e6`, retained in `evidence/color_constructor.asm`). Its copy path resolves the pseudocode's inline assembly. A temporary 15-second Ghidra startup timeout was restored to its original null setting afterward.

Recovered rule applied to `src/lua_api.lua`: when `luaL_testudata` recognizes `app::Color`, instructions `0x3c54f6..0x3c5509` copy bytes **[0,40)** into independent destination storage, through **16+16+8-byte** loads/stores. Thus the representation tag and payload survive copying; the constructor does not reconstruct this branch through RGB getters. `fromGray` stores tag **4**, signed32 gray at **+8**, and signed32 alpha at **+12**, without clamping. The gray and alpha getters have direct storage paths, as shown in the fallback reads and earlier assembly audits below.

For the supported gray representation, copying must preserve **Gcopy=Gsource**, **Acopy=Asource**, and subsequent mutation must not alias the source. Existing gray packing remains **P=(G&255)|((A&255)<<8)**. Example: `Color{gray=2147483647,alpha=258}` copied into another Color retains gray **2147483647**, alpha **258**, and packs **0x02ff**; painting its extracted gray/alpha yields **[255,255,255,2]**. Previously copying discarded gray representation, and the RGB gray calculation wrapped the signed32 sum of identical channels to **-2**, giving gray **-1**. The new private weak-key state registry preserves the represented gray/index values on copies; each object's mutable channel array and representation variables remain separate.

Regression `tests/native_color_copy.rs` executes the real embedded Lua interpreter, checks stored state and independent mutations in both directions, and checks actual painted pixels. It failed before the correction with `copy lost gray representation` and passes afterward. Fresh `cargo test --offline --locked`: **112 passed, zero failed** (`checks/native-color-copy-tests.log`). `cargo fmt --check` and `cargo build --offline --locked` passed (`checks/native-color-copy-build.log`). Craft Anything `rebuild_check` also passed build, desktop startup/screenshot (938×990), and mechanics validation (28 entries: 27 sourced, one estimated).

Scope: verified correction covers stored gray value copying and independent mutation. HSV/HSL representation preservation, palette-dependent indexed behavior, mixed constructor precedence and out-of-range gray-to-RGB conversion remain unresolved. Full original application parity is not claimed.

## Current native audit — grayscale table construction (2026-11-02)

Verification: `cargo test --offline --locked` passed **111 tests, zero failures** (`checks/native-gray-constructor-tests.log`). `cargo fmt --check` and `cargo build --offline --locked` passed (`checks/native-gray-constructor-build.log`). Craft Anything `rebuild_check` passed build, desktop startup/screenshot (938×990), and mechanics validation (**28 entries: 27 sourced, one estimated**).

Freshly opened the original main executable `aseprite` with `core_target_open` in Deep mode. SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. This audit extends the earlier recovered drawing, timing, blending, preferences and scripting rules retained below.

Searched names, settings and messages with `native_strings_find`, regex `^gray$|^color_bar$|invalid_fg_or_bg`; all five matches returned (`ev_63e23594a7afd74ca9aa11b44ce882163b8b81ae9d14b289bea7b496bda0b1f4`). Followed gray string `0xf600c9` through its reference at `0x3c5a28` to `Color_new` entry `0x3c54a0` (`ev_9a6da2cc03b4d2699b5a0dafdac04db8ed93a982fd64cca130a4f7636615424c`, `ev_e4765b6d0bb9f4875358fe6a81bb95e60dbfe57573796d0f0021ffd044417313`). Its outgoing references identify the gray factory and two distinct numeric conversions (`ev_987616d99e92a4c7decfef8462048c61c865fe1f79087deac94622bf77caa942`); number conversion references identify its helper (`ev_4785942a665bf7a3ae24685538c2816aebacbdf82c300201d5443dda2b55cf4d`). The setting and alert are search anchors, not evidence for additional behavior changes.

Completed six sequential `native_function_pseudocode` calls, awaiting and reading each result before requesting the next:

| Function | Address | Completed pseudocode evidence |
| --- | --- | --- |
| Color_new | `0x3c54a0` | `ev_e3391ea5d6f591f7440051acc25ba72878947603d4cfcf7370cb554470dcd309` |
| Color::fromGray | `0x16a620` | `ev_46b28f301ef06ad6f24ad99a51d0496f5101113c40ae3400056889c819a7e16b` |
| lua_tonumberx | `0x53e9c0` | `ev_0cefa7aefcce3979cffd29e6e0e1fca89c748059790f8cc06b2f7929eec1b44f` |
| lua_tointegerx | `0x53eab0` | `ev_78c9a310d50d382a08c8075b32d95dfb2ca81997767d81b8e819b090e8d6f085` |
| luaV_tonumber_ | `0x554760` | `ev_67312044feded002482ea7d03de7f89946bca8fb8bb7e72a2ff911c36816479e` |
| luaV_tointeger | `0x554930` | `ev_7dd36e27cd0eb6dae6de9ba81c4d37681caf8ae8b8398562a3f91b06bf401f51` |

Ghidra timed out. These are completed built-in fallback decompilations, with approximate register types/conditions. Read the full 449-instruction constructor assembly too (`ev_e15bd624ea78eb0761226710ff962f1b10fe28ec27bdbc385597543d6746b2e6`, retained in `evidence/color_constructor.asm`). Temporarily limited Ghidra startup timeout to 15 seconds and restored null afterward.

Recovered rules applied to `src/lua_api.lua`:

- The explicit gray table branch reads `gray` and **long-form `alpha` only**. Missing/nil alpha defaults to **255**; short `a` does not substitute. Non-nil false is supplied input and converts to zero.
- Gray goes through `lua_tonumberx` (number/numeric string to double, failed conversion to **0**), then instruction `CVTTSD2SI esi,xmm0` at `0x3c5ac0`. Thus **G=trunc_toward_zero(double(gray))** for representable signed32 results. In the usual masked-invalid x86 execution environment, NaN, infinity and out-of-range conversion yield **-2147483648**. Truncation happens before the range decision: `2147483647.9` gives **2147483647**, while `2147483648` gives **-2147483648**; `-1.9` gives **-1**, unlike gray assignment's floor rule.
- Alpha uses the existing recovered integer rule: integer identity, otherwise floor of finite numeric input within signed64 range, failed conversion zero; then **A=signed32(I(alpha)&0xffffffff)**. Construction does not clamp it. For example `'258.9'` stores **258**, false stores **0**, and `0x100000063` stores **99**.
- `fromGray` writes representation **4**, G at offset **+8**, A at **+12**, without saturation. Existing `grayPixel` packing retains low bytes: **P=(G&255)|((A&255)<<8)**. `{gray='258.9',alpha='258.9'}` therefore packs gray **2**, alpha **2**. The build previously retained fractions, strings, false and unconverted alpha and incorrectly accepted short `a` here.

The two new regression tests run the real embedded Lua interpreter and paint document pixels from the packed gray value. Both failed before correction at their conversion checks. Coverage includes negative fractions, numeric strings, false, NaN, signed32 boundaries, missing alpha, conflicting short alpha, and wide integer alpha. Alternate Color representations/copying, mixed gray/index/HSV table precedence, and native out-of-range gray-to-RGB conversion remain outside this correction; full application parity is not established.

## Current native audit — Lua gray assignment (2026-11-02)

Freshly opened the original main executable `aseprite` with `core_target_open` in Deep mode. SHA-256 remains `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Prior audits below retain the recovered defaults, frame timing, line/fill, blending, conversion and scripting rules.

Searched names, settings and messages using `native_strings_find`, regex `^gray$|^color_bar$|invalid_fg_or_bg`: all five matches returned (`ev_ca8d181fc73d1684c9e92ba86e3e04473139f03d6b4f0645312ac724555afa60`). Followed gray string `0xf600c9` to constructor reference `0x3c5a28` (`ev_45e941e9640ecfe1a3d9764ccd1d351b4efe37733fca5f7c70ef0c5d52fe21fa`). Gray property setter `0x3c6920` has a registration pointer at `0x1cbafb8` (`ev_3defc80b68928610107709deb0f5733b226bf06297ce97f06221111526a703b2`). Followed its outgoing calls to integer conversion, getAlpha and fromGray (`ev_4d3b1ef7e5511d49f852c0d9c33c88ffd48cadfed332270cb3517da3ea66eefd`). Setting/alert strings are navigation anchors, not evidence of new settings/alert behavior.

Requested and read six `native_function_pseudocode` results sequentially, awaiting each completed response before the next:

| Function | Entry | Completed pseudocode evidence |
| --- | --- | --- |
| Color_set_gray | `0x3c6920` | `ev_5f28e1a29d293f49fc69605161ce6a995787e569dcade83312d0fa879d2a709c` |
| Color::fromGray | `0x16a620` | `ev_46b28f301ef06ad6f24ad99a51d0496f5101113c40ae3400056889c819a7e16b` |
| Color::getGray | `0x16c2b0` | `ev_cb4b93a186f1ca43bf524dca5698f19da1edac3405b8c0dbc13919067ec6c73c` |
| Color_set_red | `0x3c5c90` | `ev_3cad851c15c3460595015a583795efbab2d32fa6b7df7e92a3b31daa8ab76c06` |
| Color::getAlpha | `0x16b2a0` | `ev_410e7153101ea7480fb0a868ee24d4c43cca5c01a933bb6cbe70db57dab4facb` |
| lua_tointegerx | `0x53eab0` | `ev_78c9a310d50d382a08c8075b32d95dfb2ca81997767d81b8e819b090e8d6f085` |

Ghidra timed out; all six completed results use the built-in fallback, with approximate register/type/control-flow recovery. Temporarily used a 15-second Ghidra startup timeout and restored null afterward. Read complete assembly for setter, factory and gray getter; saved `evidence/color_gray_assignment.asm` (evidence `ev_356d9207c12c3cc4500e388f942564f78e9e61b45063b96b5fe658ebbe2aee59`, `ev_e763d1c18e0dcd693a385c4f175006b4e517d2d1b5c517daca6e4c13781999b6`, `ev_4bb3a220e8d388fdec0d2a0bf2e325b77f97d12af028cf81723e6a20770c1521`).

Recovered rules now used in `src/lua_api.lua`:

- Assignment `c.gray=v` uses `G=signed32(I(v)&0xffffffff)`, with the previously recovered integer conversion I (floor for finite numeric inputs/numeric strings, failure zero). The setter narrows with `mov esi,ebx`, preserves `getAlpha()`, and replaces the Color using `fromGray(G,A)`. Factory stores type **4**, G at **+8**, A at **+12**, without saturation. Gray getter's direct-storage path returns G; grayPixel retains the previously recovered low-byte packing `(G&255)|((A&255)<<8)`.
- Corrected byte saturation on gray assignment: `-1.2` stores **-2** and packs **254**; `'258.9'` stores **258** and packs **2**; `0x100000063` stores **99**; false converts to **0**. Alpha **258** is retained and packs **2**.
- Native RGB setters reconstruct the Color with `fromRgb`, replacing the gray representation. Clear the Lua gray state on RGB assignment, so gray **40**, alpha **99**, then alpha **88** and red **200**, yields RGB **[200,40,40,88]** and recomputed gray **120**, rather than stale gray **40**. Alpha-only assignment retains the gray representation.

Both embedded-runtime regressions failed before correction: signed state assertion failed, and gray-to-RGB mutation painted **[40,40,40,88]** instead of **[120,120,120,88]**. Both pass after correction. This is a focused assignment correction; alternate representations, constructor/copy type preservation, HSV/index mutations and full application parity remain unresolved. Native gray out-of-range RGB conversion is not established by these tests.

Fresh verification: all **107 tests passed**, zero failures (`checks/native-gray-assignment-tests.log`); `cargo fmt --check` and `cargo build --offline --locked` passed (`checks/native-gray-assignment-build.log`). Craft Anything `rebuild_check` passed build, desktop startup/screenshot (938×990), and mechanics validation (27 values: 26 sourced, one estimated).

## Current native audit — Lua RGB grayscale properties (2026-11-02)

Opened `aseprite` again with `core_target_open` in Deep mode. SHA-256: `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. The previous audits below retain the recovered drawing, timing, blending, settings and constructor rules.

`native_strings_find` searched names, settings and messages with `^grayPixel$|^gray$|^color_bar$|invalid_fg_or_bg` and returned all six matches (`ev_7348714f1d8b0ab7cdcb872223addd8e4bd4fddebd60f9a1b783c5adafcc9f32`). Followed `grayPixel` at `0xf3fbe7` through `native_xrefs_list` to property registration `0x1cbafd8` (`ev_49720b40f2dee56b4fa41299a77678c050b107fd6a5d2fcda782a7d0be0420a4`). Its getter pointer at `0x1cbafe0` is `0x3c6a20`, followed by a null setter. Outgoing references lead to `color_for_target_mask` (`ev_87e6df250bcc836d02573cc7861b5b4cd7ac521c4c8f7f92efa0551c24ca3fc3`), then named references locate `Color::getGray` and the RGB-to-HSL constructor (`ev_6cf018b3d3cbf18d1a37147cb57f407af239301689ccd097c6683e023772f425`, `ev_de28f65a87b3ae766010999bc2bd3c5860fe91a3ed64c1944f294c6b59a0b169`). The setting and alert strings are navigation anchors; this audit makes no new behavior claims about them.

Read seven sequential `native_function_pseudocode` results, awaiting completion and reading each before requesting the next:

| Function | Entry | Completed pseudocode evidence |
| --- | --- | --- |
| Color_get_grayPixel | `0x3c6a20` | `ev_83e556a462e586246746ec902ef1327e4b5dc8af53ba1c97c63702001d9b98a4` |
| color_utils::color_for_target_mask | `0x16dc90` | `ev_81ff27c9fa2eef77eb90f338c9be7f72be01081fd912576bce5d73f3dd93d69e` |
| Color::getGray | `0x16c2b0` | `ev_cb4b93a186f1ca43bf524dca5698f19da1edac3405b8c0dbc13919067ec6c73c` |
| gfx::Hsl::Hsl(Rgb const&) | `0x6913b0` | `ev_06449ab7507c36c2a8b586137de8358ba30c262c7019c03032beab4787c89ee7` |
| gfx::Rgb::maxComponent | `0x691a30` | `ev_7e35986139593a464677ec61e59228167285070385823f419fcfa66309cd9fdc` |
| gfx::Rgb::minComponent | `0x691a50` | `ev_ef8ed1e80021affe74990ae4913bb4721523f73bd614781a6bba6e85773efbd3` |
| Color_get_gray | `0x3c68f0` | `ev_f9fabce67a8be45e9d82b985957d56caf8e01e535cb03cd42c453e03445b0c53` |

Ghidra timed out; completed results use the built-in register-level decompiler. Its conditional moves flatten in the min/max pseudocode and floating operations appear as inline assembly. Checked the actual signed `cmov` instructions, complete HSL constructor, gray conversion and packing instructions, and double constants; saved them in `evidence/color_gray.asm`. Temporarily used a 15-second Ghidra startup timeout and restored its original null value.

Recovered rules now used by `src/lua_api.lua` for RGB Color properties:

- Let `hi=max(R,G,B)`, `lo=min(R,G,B)`, and `S=signed32(hi+lo)`. The native HSL constructor computes double `L=(S/255.0)*0.5` at `0x6913d2..0x691403`, storing lightness at `+16`. Constants are **255.0** at `0xf7c200` and **0.5** at `0xf7b498`. `Color::getGray` loads that lightness and returns `G=trunc_toward_zero(L*255.0)` using `CVTTSD2SI` at `0x16c3a2..0x16c3b0`. Preserve the floating evaluation order. For ordinary channels this is approximately `(hi+lo)/2` truncated; all three pure primaries yield **127**.
- `Color_get_gray` exposes this integer as a Lua number. `grayPixel` selects grayscale target format **1**, mask **0**; the helper calls `getGray` and `getAlpha`, then packs `P=(G&255)|((A&255)<<8)` at `0x16dd1b..0x16dd37`. It recomputes per read and remains read-only. Constructor alpha **258** therefore packs to **2**.
- Corrected the RGB `gray` getter's previous weighted-luma calculation and `grayPixel`'s previous direct-red packing and alpha saturation. This API conversion uses HSL lightness; the separately recovered sprite conversion still uses its own luma formula.
- Existing explicit-gray, indexed, HSV/HSL object representation, type-changing mutations and copying retain their prior implementation limits. This correction and its tests establish RGB property behavior, not full Color/API or ecosystem parity.

Both regressions exercise the real embedded Lua interpreter and painted document pixels. Before correction, primaries painted gray **54/182/18**, and packed green with alpha 258 painted **[0,0,0,255]**. After correction, primaries paint **127/127/127**, packed green paints **[127,127,127,2]**, and an RGB mutation to **[10,20,30,99]** packs/paints **[20,20,20,99]**. Both tests now pass. Fresh `cargo test --offline --locked`: **105 passed, zero failures** (`checks/native-gray-tests.log`); `cargo fmt --check` and `cargo build --offline --locked` also pass (`checks/native-gray-build.log`). Craft Anything `rebuild_check` also passes build, desktop startup/screenshot (938×990), and mechanics validation (26 values: 25 sourced, one estimated).

## Current native audit — RGB table construction (2026-11-02)

Freshly opened the original main program `aseprite-reference` with `core_target_open` in Deep mode. SHA-256: `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. The earlier audits below continue to document drawing, timing, blending, settings and other recovered behavior.

`native_strings_find` searched names, settings and messages with `^Color$|^gray$|^color_bar$|invalid_fg_or_bg`, returning all nine matches (`ev_ddb5c202457d618cbd33862584c035349d8c27cc3768fe55aabde1791723197e`). Followed `gray` at `0xf600c9` through `native_xrefs_list` to the constructor reference at `0x3c5a28` (`ev_9a6da2cc03b4d2699b5a0dafdac04db8ed93a982fd64cca130a4f7636615424c`); `native_function_find` resolved its entry to `0x3c54a0` (`ev_e4765b6d0bb9f4875358fe6a81bb95e60dbfe57573796d0f0021ffd044417313`). Outgoing calls connect that constructor to `Color::fromRgb` and `lua_tointegerx` (`ev_288c0e4475a90d6f265432a51d36b8427f8a92b67694821fd31f46217dd043a0`); the latter calls `luaV_tointeger` (`ev_b28793b0187f347b6222abf9101486dc88c1a6b918202366d8ecea6cccd03f11`). The setting and alert are search anchors, not evidence for additional implemented behavior.

Completed four sequential `native_function_pseudocode` requests, awaiting and reading each result before requesting the next:

| Function | Entry | Completed pseudocode evidence |
| --- | --- | --- |
| Color_new | `0x3c54a0` | `ev_e3391ea5d6f591f7440051acc25ba72878947603d4cfcf7370cb554470dcd309` |
| Color::fromRgb | `0x16a5c0` | `ev_c724d1688af7d34962c3e4f0afc5742839218376fffd3d2f0d75d454089a1df7` |
| lua_tointegerx | `0x53eab0` | `ev_78c9a310d50d382a08c8075b32d95dfb2ca81997767d81b8e819b090e8d6f085` |
| luaV_tointeger | `0x554930` | `ev_7dd36e27cd0eb6dae6de9ba81c4d37681caf8ae8b8398562a3f91b06bf401f51` |

Ghidra timed out; these completed responses use the built-in register-level decompiler. Conditional/type recovery is approximate. Also read all 449 constructor instructions (`ev_e15bd624ea78eb0761226710ff962f1b10fe28ec27bdbc385597543d6746b2e6`), retained as `evidence/color_constructor.asm`, to confirm field selection and 32-bit argument narrowing. Temporarily limited Ghidra startup to 15 seconds, then restored the original null setting.

Recovered rules now used in `src/lua_api.lua`:

- If table `r` is non-nil (including false), select **only** `r/g/b/a`; otherwise, if `red` is non-nil, select **only** `red/green/blue/alpha`. Missing RGB fields convert to zero. Missing/nil alpha defaults to **255**. Fields in the other family do not supply missing components. RGB forms take priority over indexed/gray/HSV forms.
- For each supplied channel, use `J(v)=signed32(I(v)&0xffffffff)`, where the previously recovered `I` uses integer identity, floor conversion of finite numbers/numeric strings within signed64 range, and zero on failed conversion. `Color::fromRgb` writes the four signed32 values directly at offsets `+8,+12,+16,+20`; it does **not** saturate alpha or RGB during construction. This differs from subsequent alpha assignment's clamp.
- Copying an RGB Color retains its channel values in independent state. Packing through the previously audited `rgbaPixel` getter masks each channel to its low byte: constructing `{red=-1.2,green='258.9',blue=0x100000063,alpha=258}` retains **[-2,258,99,258]** and packs to **[254,2,99,2]**.
- Fixed constructor field mixing and saturation, preserved RGB values when copying, and shared signed32 conversion with the existing setters. Internal array colors and string conveniences remain supported. Numeric constructor overloads, empty-table defaults, gray/index/HSV/HSL state copying, exact alternate-color conversions, and full Color parity are outside this correction.

Both new embedded-Lua regressions failed before the fix: native integer state assertion failed, and mixed-field construction painted `[1,2,3,4]` instead of `[1,0,0,255]`. Both pass after the fix. Fresh verification: all **99 tests** pass (`checks/native-constructor-tests.log`); `cargo fmt --check` and `cargo build --offline --locked` pass. Craft Anything `rebuild_check` passes build, desktop startup/screenshot (942×1005) and mechanics validation (24 values, 23 sourced and one estimated). These checks establish this correction and a working build; full ecosystem parity remains unresolved.

## Current native audit — mutable Lua RGB Colors (2026-11-02)

Freshly opened the original main executable `aseprite-reference` with `core_target_open` in Deep mode; SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Prior audits below remain evidence for their respective mechanics.

Searched names, settings and messages with `native_strings_find`: a broad `Color|opacity|invalid` search returned 985 matches (only the first 100 were requested), then the focused `^red$|^rgbaPixel$|^color_bar$|invalid_fg_or_bg` search returned all four matches (`ev_0a7e9dbdd8ab73cd48eaf7ef4c592ec3d595a842875bab49c777b90a79f80e68`). Followed `rgbaPixel` at `0xf3fbdd` to property registration `0x1cbafc0`; its getter pointer at `0x1cbafc8` is `0x3c69b0`, with a null setter. `native_xrefs_list` evidence: `ev_360aca6d0b90a21d746d6a96711081e36291e9aa0941d13222a782d4130f7749`. The red property registration at `0x1cbae40` maps to getter `0x3c5c60` and setter `0x3c5c90`. Followed setter outgoing references to integer conversion, other channel getters and `Color::fromRgb` (`ev_761c572878c4a7e1e0e2ca77b25f30aa465df0c57c0f114aba9ce28f0930df29`). Registration and arithmetic instructions are saved in `evidence/color_mutation.asm`.

Completed eight sequential `native_function_pseudocode` requests, waiting for and reading each result before requesting the next:

| Function | Address | Completed pseudocode evidence |
| --- | --- | --- |
| Color_get_rgbaPixel | `0x3c69b0` | `ev_7105ca60f7d8b9ede0e31635a0e90e9346c63858a01e2b66a620d411433cd0b0` |
| color_utils::color_for_target_mask | `0x16dc90` | `ev_81ff27c9fa2eef77eb90f338c9be7f72be01081fd912576bce5d73f3dd93d69e` |
| Color_set_red | `0x3c5c90` | `ev_3cad851c15c3460595015a583795efbab2d32fa6b7df7e92a3b31daa8ab76c06` |
| Color_set_green | `0x3c5d70` | `ev_e24b968a9bf6321264369bce64bbf4662482bd90a8bb2a4d9b6b1c463e222b6c` |
| Color_set_blue | `0x3c5e50` | `ev_ca0b4159576582b55bad00ff592cdf63272b5804e151c7d4fa6a810a4ad665d4` |
| Color_set_alpha | `0x3c5f30` | `ev_5fdb5589a65f20c765770d74240d221fcb5d0df14c5cc9b079b1a5c7016d0982` |
| Color::fromRgb | `0x16a5c0` | `ev_c724d1688af7d34962c3e4f0afc5742839218376fffd3d2f0d75d454089a1df7` |
| Color::setAlpha | `0x16cdc0` | `ev_754e4df2c6d200f5a53d46eba3a46c8f6ad2c43e66e52968de16bcecd8ce1eb8` |

The first getter request timed out at the tool transport's 300-second limit and did not produce pseudocode. Retried with a temporary 15-second Ghidra startup timeout, then restored the original null setting. All eight completed results use the built-in fallback after Ghidra timed out. Its register variables and conditional moves are approximate: notably, the alpha clamp is established by the actual `test/cmovg/cmp/cmovl` instructions at `0x16cdc9..0x16cdd9`, not by trusting the fallback's flattened assignments.

Recovered rules applied in `src/lua_api.lua`:

- Channel assignment converts through the previously recovered `lua_tointegerx` floor/failure-zero rule, then narrows to signed 32-bit: `J(v) = signed32(I(v) & 0xffffffff)`. RGB setters replace just their channel and preserve the other three. `Color::fromRgb` stores the integer channels without saturation. Thus red `-1` stays -1 in RGB Color state, and red `0x100000063` becomes 99.
- Alpha assignment uses `alpha = min(255,max(0,J(v)))`; it preserves RGB. This clamp differs from the low-byte mask used by packed pixel helpers.
- `rgbaPixel` is read-only and computed per read: `(R&255) | ((G&255)<<8) | ((B&255)<<16) | ((A&255)<<24)`. The getter builds an RGB ColorTarget with mask zero; the packing instructions at `0x16dce8..0x16dd37` retain the low bytes. It is not a cached constructor result.
- Replaced duplicated table channel fields and cached packed value with shared channel state. Existing short aliases `r/g/b/a` are retained as a PixelCraft convenience and access the same state as `red/green/blue/alpha`; this audit does not establish native short-property aliases. Color construction defaults remain as previously implemented; full constructor parity, HSV/HSL/indexed/tile Color mutation, direct drawing of out-of-range Color tables and events remain outside this correction.

Two real embedded-Lua document-paint regressions failed before the correction: red mutation painted `[1,2,3,4]` instead of `[99,2,3,4]`; green/blue/alpha mutation likewise painted the old constructor pixel. Coverage includes all four channels, signed 32-bit narrowing, failed conversions, alpha clamping and packed byte masking. Final verification: all **76 tests** passed; `cargo fmt --check` and `cargo build --offline --locked` passed. Craft Anything `rebuild_check` passed build, desktop startup/screenshot (942×1005, inspected), and mechanics validation (23 entries: 22 sourced, one estimated).

# Pixel editor reconstruction

## Current native audit — Lua foreground/background getters (2026-11-02)

Freshly opened the original main ELF `aseprite-reference` with `core_target_open` in Deep mode. SHA-256: `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. This audit extends the drawing, timing, blending and packed-color formulas documented below.

Searched names, settings and messages with `native_strings_find`, regex `fgColor|bgColor|fg_color|bg_color|invalid_fg_or_bg`: all 13 matches returned (`ev_82c0331472dbc64c437b187ee99c534b94233ab956c17d4c30608329ae6bdc46`). Followed `fgColor` at `0xf46f8b` to registration data `0x1cd53d8` (`ev_668a5b52c73b07fc82006f743213480eecaab5ee2f7da2201bdead77af770777`). Its next pointer at `0x1cd53e0` is the getter `0x50ac90`, corroborated by incoming references (`ev_4c934f1e9c86981160ed82dc61edb774b4c57a261520b54419661ff0e1a4eea6`). The next property entry maps `bgColor` at `0xf35713` via `0x1cd53f0` to getter `0x50ae00` at `0x1cd53f8`. Registration bytes are in `evidence/lua_color_registration.asm`. Function search resolved both getters and the preferences constructor (`ev_cdbfcaadaac167cdbe5e2f26e6c54038e7cfbe652e1fea4af20a58c73178a259`). Followed outgoing getter calls to the Preferences singleton and fresh Lua userdata allocation (`ev_0db17c4767368d2dc8f0433f51ca1605bde553062807ad62be9ee5cf90c08e32`). Setting strings `fg_color`/`bg_color` directly reference the preferences constructor; the invalid-color alert is a search anchor, not a new implemented alert rule.

Completed three sequential `native_function_pseudocode` calls, awaiting and reading each response before requesting the next:

| Function | Entry | Completed pseudocode evidence |
| --- | --- | --- |
| App_get_fgColor | `0x50ac90` | `ev_dd3e21f9df2a8bbdb3a24fb25f9af88c21424bdc2aeba043dbffd052ee0ed767` |
| App_get_bgColor | `0x50ae00` | `ev_8ad8c2afe2eadf8448626ef623d57ef4be2b096cfba254d2d81f5410f72fb0f3` |
| GlobalPref::ColorBar constructor | `0x366d20` | `ev_9daca0c10dc2a4202b55b144bc919cb60962a332d5076b5a9037bf5664fef4ea` |

Ghidra timed out; all three completed responses used the built-in register-level decompiler. Its inferred types/flags are approximate. Temporarily set startup timeout to 15 seconds and restored the original null setting. Getter assembly is retained in `evidence/lua_foreground_getter.asm` and `evidence/lua_background_getter.asm`; factory assembly remains in `evidence/color_factory_defaults.asm`.

Recovered rules and implementation:

- Foreground getter copies **40 bytes** from `Preferences::instance()+0x4730`; background getter copies **40 bytes** from `+0x4820`. Each allocates fresh `0x28`-byte Lua userdata, copies the Color value, attaches its metatable, and returns one value. The results are copies; mutating a returned Color does not change the preference. These byte offsets describe native Color objects, not packed RGBA bytes.
- Factory constructor calls `Color::fromRgb(255,255,255,255)` for foreground and `Color::fromRgb(0,0,0,255)` for background. Therefore standalone RGBA scripts use opaque white foreground and opaque black background.
- Fixed `src/lua_api.lua` to copy invocation colors on every getter read. The desktop script worker now passes `[editor.foreground, editor.background]`, retaining all four channels. `extensions::run_source_with_colors` carries the snapshot into the embedded runtime; standalone calls and command discovery use the native factory defaults.
- The worker uses an invocation snapshot while editing is blocked. This audit establishes RGBA getter values/copy semantics; script color setters, indexed/tile Color representations, full Color-object mutation semantics and color-change callbacks remain unsupported. It does not establish complete extension or format parity, nor exact ellipse masks.

Both regressions were run before the getter correction: factory painting produced `[0,0,0,255]` instead of `[255,255,255,255]`; selected-color painting produced `[0,0,0,255]` instead of `[12,34,56,78]`. Tests paint real document pixels through embedded Lua and verify that mutating a returned copy leaves subsequent getter reads unchanged. Final verification: all **74 tests** passed; `cargo fmt --check` and `cargo build --offline --locked` passed. Craft Anything `rebuild_check` passed build, desktop startup/screenshot (942×1005, inspected), and mechanics validation (22 entries: 21 sourced, one estimated).

## Current native audit — Lua packed RGBA colors (2026-11-02)

Opened the original main ELF `aseprite-reference` with `core_target_open` in Deep mode. SHA-256: `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. This is a fresh audit of the existing scripting color helpers; earlier audits below remain applicable to the other implemented mechanics.

`native_strings_find` searched names, settings and messages using `rgba|fgColor|bgColor|color_bar|invalid_fg_or_bg`, returning all 69 matches (`ev_e7798ce8b0b02e4082bb7714d30d2d14a9f32ace0e9ba4a698e62787be3a7a6c`). These include `rgbaR` at `0xf4074b`, the `color_bar` setting at `0xf3e1d6`, and message `alerts.invalid_fg_or_bg_colors` at `0xf3976b`. Followed `rgbaR` references to registration data `0x1cbc400` (`ev_9518e71c93f3ceee37c360fdd9040c05a6e9fd75478a098fdd74134e1932ac3d`); the adjacent pointer at `0x1cbc408` identifies `PixelColor_rgbaR` at `0x3e21a0` (function references `ev_a350779f7b3cf807cc57248d49ee70ba7bfd578b9332aea9ad8463b0457cfd86`). The preceding registration pair at `0x1cbc3f0` maps `rgba` to `0x3e2100`. Registration bytes are retained in `evidence/pixelcolor_registration.asm`. Function search resolved the packer, all four extractors and integer helpers (`ev_41169d8eb97db1e1331713175a3462c29c714d0d5cca246907e34936e60b059d`). Followed packer outgoing calls to `lua_tointegerx`, `lua_type`, and the result push (`ev_1824bc8c565b9d722f0b7171ee983c5c601624258ce63ea71a9889825d8d2c31`).

Completed **eight sequential `native_function_pseudocode` reads**, awaiting and reading each response before the next request:

| Function | Entry | Completed pseudocode evidence |
| --- | --- | --- |
| PixelColor_rgba | `0x3e2100` | `ev_0c362b05d308707e98d280b99ead05cd3f06215fe946c2f04c2aba13f67cecd1` |
| PixelColor_rgbaR | `0x3e21a0` | `ev_6679d8856744d12a5c04427e77136bab9a4b45493530b7697cd9688d1cb215e4` |
| PixelColor_rgbaG | `0x3e21d0` | `ev_93ede02e8ad451b6acbe66014b0821404dbe7d0dfab44868d06d1a90aec6dc60` |
| PixelColor_rgbaB | `0x3e2200` | `ev_aeef9fe2cf129246b64b922da3585e5fec9cf13396a81eea4d9e3e95f105af83` |
| PixelColor_rgbaA | `0x3e2230` | `ev_180a8ecf74a8677d94dd5aa73026c2f3cd32897f3db7d61302f6708b6ac7c057` |
| lua_tointegerx | `0x53eab0` | `ev_78c9a310d50d382a08c8075b32d95dfb2ca81997767d81b8e819b090e8d6f085` |
| luaV_tointeger | `0x554930` | `ev_7dd36e27cd0eb6dae6de9ba81c4d37681caf8ae8b8398562a3f91b06bf401f51` |
| lua_type | `0x53e270` | `ev_eb33375a0c370524320ba8c1b3f49d6b5227bf86ff676e694199bac59177ceee` |

All returned the built-in fallback because Ghidra timed out. Temporarily used a 15-second startup timeout and restored its original null value after the reads. Two helper evidence ids match the earlier opacity audit: these are fresh completed calls returning the same content-addressed evidence, not new unique evidence. Register-level types/flags in fallback output are approximate. Corroborated packer and all extractor arithmetic against the full instruction listing in `evidence/pixelcolor_rgba.asm`; integer-helper assembly is retained from the earlier opacity audit in `evidence/lua_tointegerx.asm` and `evidence/luaV_tointeger.asm`.

Recovered rules, implemented in `src/lua_api.lua`:

- Let `I(v)` preserve signed 64-bit integer inputs; parse convertible numeric strings; floor finite floating inputs into the signed 64-bit domain; return **0** for failed conversions (invalid strings/types, NaN, infinity, or out-of-range floats). The helper initializes its output to zero and calls conversion mode 1. Preserve integer inputs before floating range checks so the valid integer `0x7fffffffffffffff` is not mistaken for +2^63.
- Let `U(v)=I(v)&255`. `rgba(r,g,b,a)=U(r) | (U(g)<<8) | (U(b)<<16) | (A<<24)`, where **A=255 for omitted/nil alpha**, otherwise `A=U(a)`. Missing RGB inputs convert to zero. There is **no saturation clamp**: at `0x3e2168` red is zero-extended from its low byte, at `0x3e216c..0x3e2170` green is shifted/masked, at `0x3e2177` blue is zero-extended, and the 32-bit alpha shift at `0x3e215e` retains only its low byte. The packed result is a nonnegative integer in 0..4294967295.
- Extractors are `rgbaR(c)=I(c)&255`, `rgbaG(c)=(I(c)>>8)&255`, `rgbaB(c)=(I(c)>>16)&255`, and `rgbaA(c)=(I(c)>>24)&255`. All call the same integer conversion before extracting. Invalid inputs return zero instead of raising a Lua bitwise-conversion error.
- Examples: `rgba(-1,256,511,258)` gives channels **[255,0,255,2]**; `rgba('257.9',-0.1,258.9)` gives **[1,255,2,255]**; false alpha gives zero, while nil alpha gives 255. `rgba(1,2,3,4)=0x04030201`.

Changed the exposed `app.pixelColor.rgba` and four extractors to use these rules. Internal normalized color-table packing remains separate. This audit does not establish complete Color-object behavior, grayscale/tile packing API coverage, or exact brush ellipse masks. The compact named Colors selector and artwork swatch list are project-specific UI; these are not claimed as native mechanics.

Regression `lua_pixel_color_wraps_channels_and_converts_like_native` first failed through an actual script-driven document pixel edit: [0,255,255,255] instead of native [255,0,255,2]. It also covers fractional/numeric strings, omitted/nil/false alpha, failed conversion, large exact integers, packed result layout, and extraction. Final verification: all **72 tests** passed, `cargo fmt --check` passed, and `cargo build --offline --locked` passed without warnings. Craft Anything `rebuild_check` passed build, desktop launch/screenshot (942×1005), and mechanics (21 entries: 20 sourced, one estimated). The screenshot was inspected. Complete original file/extension ecosystem support and exact ellipse masks remain unresolved.


## Current native audit — shared drawing colors (2026-11-02)

Freshly opened the original main ELF `aseprite-reference` with `core_target_open` in Deep mode. SHA-256: `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. This extends the earlier mechanic audits retained below.

Searched names, settings and messages with `native_strings_find`: SwitchColors/default/reset/background names (4 matches, `ev_eadb1a6d085eb11ad148a8109d297b79461505735c596557c2b850ba50659231`); fg/bg/default color names (27 matches, `ev_e641f6abac12156b36619d83626f50a8b0499e111398ba8b5484208e3b7b8114`); targeted settings/events/message search (15 matches, `ev_82b21c303ece46fbbc5a9f08841be67e3961a2d100002d0c00fbd851e9afbaa5`). Relevant strings include `SwitchColors` at `0xf3575d`, `fg_color` at `0xf3a049`, `bg_color` at `0xf3a065`, `color_bar` at `0xf3e1d6`, and `alerts.invalid_fg_or_bg_colors` at `0xf3976b`.

Followed SwitchColors string references to its constructor `0x499b30` and factory `0x499dd0` (`ev_f52aef0d2da9bacdc7d5e1ceed97f5310311d01c138a9340f2b419372ab15b1e`), resolved the corresponding onExecute symbol, and followed its outgoing references to getters/setters (`ev_6c4ded9e37d49cc4c20103cdb4a0e0315868092630aede69b68b819d06a93161`). Followed `fg_color` references to the global ColorBar preferences constructor (`ev_e6960144ebc4d76e6cc150b328c7d77618ef12dcfe022a488b199d3336305fac`).

Completed **ten sequential `native_function_pseudocode` reads**, awaiting and reading each result before requesting the next:

| Function | Entry address | Completed pseudocode evidence |
| --- | --- | --- |
| SwitchColorsCommand::onExecute | `0x499b60` | `ev_092f0153100edd42a3d86b40e6d9cd193921dc9a1ec22955be728cd07d457eec` |
| ColorBar::onActiveSiteChange | `0x22f2f0` | `ev_03c8c23076485ae12a5b8862a7765e809fa2fe786fbaa3ed74ee094c3fb2f5f4` |
| ColorBar::setFgColor | `0x22d440` | `ev_ac51ac88bd358ea1ea9a32a34bbab574c3cfa6c36e9a85569a90c73b9f173add` |
| ColorBar::onFgColorButtonChange | `0x22cea0` | `ev_2dcbeb354b3038f825624bbc41f2de3e6dd2d91c999d33e9a9f9d73781f8849d` |
| ColorBar::onBgColorButtonChange | `0x22d0c0` | `ev_0316730d1edd7ddff5deb6e9c3311eb2b06d9989a35a7a1a2f4d25622bf92352` |
| gen::GlobalPref::ColorBar constructor | `0x366d20` | `ev_4871039251c8a168500125848f8100cb630856b45868ecf34e477564a78385ec` |
| Color::fromRgb | `0x16a5c0` | `ev_80feda9e2e48d068250931e8d3edad556f20f83512015aa83cbf7165f4e07e1b` |
| ColorBar::setBgColor | `0x22d3f0` | `ev_ec2e12cbeb30c98a745a7e7dc300cc888cc22af245c5a3feb3b015f4c9d5dd99` |
| ColorBar::getFgColor | `0x22de50` | `ev_1ef6ac954ae547ddec0d3a004dcd6ef0e76c0e57eaddfb156b4c10f7aac6bb1e` |
| ColorBar::getBgColor | `0x22ec00` | `ev_0e124cf4a68e3b580fd23d5dd655023204801fffe7f018d865a63942a4b207ef` |

All ten completed through the built-in fallback because Ghidra timed out. Its register/flag reconstruction is approximate. Temporarily set Ghidra startup timeout to 15 seconds for completed replies, then restored the prior null value. Confirmed the numeric constructor arguments and foreground singleton writes with supplemental `objdump` listings in `evidence/color_factory_defaults.asm` and `evidence/color_global_foreground.asm`.

Recovered rules and implementation:

- **Factory foreground = RGBA (255,255,255,255); background = (0,0,0,255).** Constructor call at `0x3670a5` supplies four 255 arguments; call at `0x36718a` supplies 0,0,0,255. `Color::fromRgb` stores those four arguments directly. These are defaults before any saved preferences. Replaced PixelCraft's coral/cream startup colors with these values. The existing Default colors button now applies the same factory defaults, instead of the reversed black/white pair. The original's separate reset-button action has not been traced; matching its exact reset workflow is not claimed.
- **Drawing colors are application-wide state.** Foreground callback calls `Preferences::instance()` at `0x22ced6` and copies the complete color to singleton +`0x4730`; background callback similarly writes singleton +`0x4820`. Both emit preference change signals. `onActiveSiteChange` handles document observers and tile UI without restoring document-specific RGB colors. PixelCraft now retains the current foreground/background, including alpha, when opening, switching or closing sprite tabs; removed these fields from SpriteTab. Per-tab artwork, history and view still remain independent. Cross-launch color persistence is not implemented by this correction.
- **Normal color swap:** `(fg',bg')=(bg,fg)` using complete saved Color objects, not only RGB channels. The original reads both colors before setting either. PixelCraft's existing full RGBA swap already satisfies this rule. Native shading reversal and tile-ID swapping are separate branches and are not implemented or claimed here.

Regression tests `native_color_defaults` and `drawing_colors_are_shared_between_sprite_tabs` both failed before correction (coral instead of white; old tab color restored instead of current shared color), then passed after correction. Final verification: all **69 tests** passed; `cargo fmt --check` and `cargo build --offline --locked` passed without warnings. Craft Anything `rebuild_check` passed build, desktop launch/screenshot (938×990), and mechanics (20 entries: 19 sourced, one estimated). The screenshot was inspected. Full original file/extension ecosystem compatibility and the previously documented ellipse-mask uncertainty remain unresolved.


## Current native audit — opacity conversion (2026-11-02)

Reopened the original main program with `core_target_open` in Deep mode: `aseprite-reference`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. This audit extends the completed earlier audits below; those earlier reads were not repeated or counted as new reads.

`native_strings_find` searched `opacity|tx.set_layer|tx.set_cel`, returning all 37 names, settings and messages (`ev_19f9a69239135a44e4af14620d4ada3d0e896ca497f9d7ed88ef59e633774325`). Examples: setting `opacity` at `0xf390d1`, setting `nonactive_layers_opacity` at `0xf3e135`, command `LayerOpacity` at `0xf42ff6`, message `Set Layer Opacity` at `0xf42fdb`. `native_xrefs_list` followed that message to `0x45097a` inside `LayerOpacityCommand::onExecute` (`ev_4ff84fabb53cb8db6338faf1fed7bcd2ca99a7f79a5b16c71b72441ca5a47d20`). Function-name search then resolved the script setters (`ev_b83f09fd36bcf2bbd7f0ec7ec9426143d6bf1a67dfabaca3bf0e0e4d79d728b7`) and opacity commands (`ev_e1e3692b3307497e5e58e37257a89eaad86607212b4b1a4554fa7263031f9734`). These searches alone establish no numeric conversion rule.

Completed **ten sequential `native_function_pseudocode` reads**, awaiting and reading every result. All used the built-in fallback; Ghidra did not become ready. The first two requests reached the transport's 300-second timeout, but their completed fallback results were recovered and read from the evidence ledger. Temporarily reducing `engines.ghidra.startup_timeout_secs` to 15 allowed direct tool responses; the setting was restored to its original null value afterward. The ten direct successful reads are listed below. Fallback register/flag reconstruction is approximate: the Layer setter's bit-test condition and the conversion helper's conditional moves are not trusted without assembly.

| Completed pseudocode read | Address | Evidence id |
| --- | --- | --- |
| Layer_set_opacity | `0x3dd4a0` | `ev_86884a0977896d1132728c6a5f51de3bf04988467462d8e6697c1c145f791041` |
| lua_tointegerx | `0x53eab0` | `ev_78c9a310d50d382a08c8075b32d95dfb2ca81997767d81b8e819b090e8d6f085` |
| luaV_tointeger | `0x554930` | `ev_7dd36e27cd0eb6dae6de9ba81c4d37681caf8ae8b8398562a3f91b06bf401f51` |
| Cel_set_opacity | `0x50e0a0` | `ev_84d3113dc778311ecc2a452fceee80214c69ff37c0252c19f36076c908627d36` |
| SetLayerOpacity constructor | `0x416140` | `ev_6c556465599887d3d9b5a0158dd2bd5d2032ed8b9ce8ad3d86717868910cc2a8` |
| SetLayerOpacity::onExecute | `0x416190` | `ev_5709d699ecc1a30f653267fdd42fe9f05e7534ec7fe7cd6f1024536a477f9a18` |
| SetCelOpacity constructor | `0x414f90` | `ev_046738ab2ddfd669ac88d9594a1384912aad1859d4717d8b8e71360bad146910` |
| SetCelOpacity::onExecute | `0x414fe0` | `ev_d75e4e079f1ba447aa9508bccdee0cdba8f6a01bf9a94d5f8de619135a50806d` |
| Cel::setOpacity | `0x5f4b00` | `ev_f8d5f715b2365795590827df46c825c4288bc8abb7f52b6118ac5c868251adeb` |
| LayerOpacityCommand::onExecute | `0x4508d0` | `ev_58e52d5351a36aca4b64e3f758e4ed0c73e5ba64208d42c6e3d4a4b3e37d05ce` |

Followed the setter's outgoing references to the integer helper and constructor (`ev_058c41e1b6fc145b311ff99c57248dd057bbaeade75caa450141d3e2c4461ff2`). Full native assembly reads completed for Layer_set_opacity, lua_tointegerx and luaV_tointeger: 92, 71 and 81 instructions respectively (`ev_51a10cdfa29bc55f3367b6c451892a486cbb40636bc444c364519e7729d67533`, `ev_b61fb1d1b48dde568a63059ea8a697ade63361618fa834b7d8ee9df71eea5715`, `ev_8c466bf72f0fa667eb554957e12414689f0c6ba20d4452ea7599785f2a643093`). Supplemental `objdump` listings are retained in `app/evidence/`. Original bytes at `0xf7b650` = `000000000000e043` encode +2^63; `0xf7b698` = `000000000000e0c3` encode -2^63, confirming the float-conversion limits.

| Function | Address | Assembly observation |
| --- | --- | --- |
| Lua `Layer_set_opacity` | `0x3dd4a0` | Calls `lua_tointegerx(L,2,NULL)` at `0x3dd4f6`, passes low 32 bits to SetLayerOpacity at `0x3dd59c`. |
| Lua `Cel_set_opacity` | `0x50e0a0` | Calls the same conversion at `0x50e171`, passes low 32 bits to SetCelOpacity at `0x50e17e`. See `evidence/cel_set_opacity.asm`. |
| `lua_tointegerx` | `0x53eab0` | Initializes output to zero at `0x53eac3`; non-integer path calls `luaV_tointeger` with mode **1** at `0x53eb9a`. Failure returns the untouched zero output. See `evidence/lua_tointegerx.asm`. |
| `luaV_tointeger` | `0x554930` | Parses numeric strings, calls floor at `0x5549b5`; mode 1 accepts the floored fraction. Only results in signed int64 range are written. Invalid type/text, NaN, infinities or out-of-range results leave the output untouched. See `evidence/luaV_tointeger.asm`. |
| SetLayerOpacity constructor / onExecute | `0x416140` / `0x416190` | Saves requested int32 at +0x18 and writes it to LayerImage +0xb8. No 0..255 clamp on this path. See `evidence/set_layer_opacity.asm`. |

Cel::setOpacity at `0x5f4b00` also writes an int32 directly to CelData +0x80 without a clamp. The two command constructors retain old/requested values for undo; onExecute increments the relevant version.

Recovered input rule: integer inputs pass through; convertible floats/strings use **floor**, not truncation; failed conversion yields **0**; setters narrow the result to int32. For example `123.9` and `'123.9'` give 123, and infinity, NaN, `'invalid'`, false, nil and a table give 0. This executable's Lua conversion mode is observed directly; a generic assumption about default Lua 5.4 conversion would be wrong here.

Changed `src/lua_api.lua` so layer/cel opacity failed conversions become zero instead of clamping positive infinity to 255 or throwing for invalid types. Successful numeric opacity in 0..255 retains the recovered floor rule. **Boundary:** PixelCraft still stores byte opacity and clamps successful values outside 0..255. The original layer setter stores int32 without that clamp; out-of-range parity and group-layer opacity remain unsupported and are not claimed by this correction.

Regression `lua_opacity_failed_numeric_conversion_becomes_zero` failed before correction at `app.activeLayer.opacity=math.huge` (255 instead of 0), then passed for both layer and cel setters. All **65 tests** and `cargo fmt --check` passed after correction. `cargo build --offline --locked` also passed. Craft Anything `rebuild_check` passed build, desktop launch/screenshot (942×1005), and mechanics (18 entries, 17 sourced, one estimated). The screenshot was inspected.

## Current native audit — scripting duration conversion (2026-11-02)

Opened the original main ELF with `core_target_open` in Deep mode: `aseprite-reference`, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Completed **seven sequential `native_function_pseudocode` reads**, awaiting and reading every result. Ghidra could not start; all seven completed through the built-in fallback, whose approximate register/flag reconstruction is not treated as sufficient for omitted argument moves or conditional moves. Read full assembly for the setter, command constructor and final clamp.

String searches covered names, settings and messages: `duration|frame_duration|Frame.*duration|Invalid.*frame|pixel_connectivity` (35 matches, `ev_77dd615b9d983d67432bcc4e3766e2140fa719c9739516235e5ae3c13a75ac10`) and `tx.set_frame_|newFrame|commands.NewFrame_` (11 matches, `ev_441c5c46cbb09ef5a83f4561e531f785076afbc25b9342904ef59cb06380f8f0`). Followed transaction message `tx.set_frame_` at `0xf401c6` to `0x3ce49d` inside the duration setter (`ev_5cd78389158c21fc03a0094ca71342a0b36d5d6433622b0edc4b68acf80a4c94`); setting `pixel_connectivity` at `0xf38b5e` to its constructor (`ev_8589c9615a5403d00cc240a24bb8ebfc11e410b1cdd9de6db835d470c3efb20a`); export warning `alerts.file_format_10ms_duration_precision` at `0xf36bce` to file-save handling (`ev_770dd9c420c0ac6e0365bbf42c45687132353f5ecf0dde4b226f43a4c0f2c559`). The warning reference alone does not establish export quantization behavior. Generic `duration` has unrelated references and was not mistaken for the Frame setter. Followed the setter's outgoing calls to `SetFrameDuration` (`ev_bb671e11a9b5f4ad617c30a82a7ca36333e0c477c036b152259f9e7307d77509`); symbol search resolved its virtual `onExecute` (`ev_fb10f8724232827712db7ab0a6ddd9c945b53b25e2fe6d5dae02acc27ad77ab4`).

| Completed function read | Address | Recovered behavior / evidence |
| --- | --- | --- |
| Lua `Frame_set_duration` | `0x3ce440` | Reads a Lua number, multiplies in double precision, converts to signed int32 with truncation before executing command. Pseudocode `ev_3f22c15620330775cce907dab0a4ff7783531be37e91569f301d264ec7be2c71`; full 75-instruction assembly `ev_aa31e01a1cf6a6d1c0f28ac6a2b28e5a9d3751947fb5fc23a0446e1d7e5ab1f9`. |
| `cmd::SetFrameDuration` constructor | `0x415570` | Saves requested int32 duration at +0x1c and old duration at +0x18. Pseudocode `ev_f73685064d5ed03c230457839386561703a04cfcea5c259e23c7aa13617ac874`; all 27 assembly instructions `ev_21cee70570458c1d71f084b977e2e1e60d1424c79aacec490a358838153f54bc` resolve the omitted `ecx` to `ebp` move. |
| `cmd::SetFrameDuration::onExecute` | `0x4155d0` | Passes requested +0x1c duration to Sprite setter and increments sprite version. `ev_b48418998976acee716869f6a78d6f6c446c1e6f2d012304c801430de55f6b19`. |
| `Sprite::setFrameDuration` | `0x61a420` | For valid zero-based frame i, stores `max(1,min(65535,int32_ms))`. Pseudocode `ev_306fda5d2ea134eb34ae1bdc260478f8ea5224238ba81714adc96eff875ea5e6`; all 14 assembly instructions `ev_191ac27d385689c1e5621fa4eb0a424e4668ed37a2667562269756a622e1b04d` establish conditional-move limits. |
| `lua_tonumberx` | `0x53e9c0` | Returns double to setter, with conversion helper for other value representations. `ev_fe6c0701ff4f9a72b021fa34f845bde1694b1e0787814575878680acbe3539dd`. This patch addresses numeric inputs only. |
| `ToolPref::Floodfill` | `0x376670` | Retains `pixel_connectivity` enum-zero default, as earlier audit. `ev_8c3ff38ff013d3407a8e58ce02489942a3c07b12a31fe73d2f7b0332a1d7b4a2`. |
| `DocApi::adjustTags` | `0x19f2d0` | Re-read insertion/deletion branches; fallback omits flag details, so no new tag rule inferred. `ev_fcac7798cb5943d0f5803eadde1dc660820777919e18f7d9515257b002baf5d3`. |

The multiplication constant at `0xf7b618` is bytes `0000000000408f40`, IEEE-754 little-endian **1000.0** (`ev_699b48c44b272501962cd2668b4046414868b581c1936ea10e907bb3224f2131`). Formula for numeric seconds s: `x = double(s)*1000`; `n = trunc_to_int32(x)`; stored duration is `clamp(n,1,65535)`. On this x86_64 executable, `cvttsd2si ecx,xmm0` at `0x3ce4ea` yields signed integer-indefinite `-2147483648` for NaN or a truncated result outside int32 range under the usual masked-invalid floating-point environment; final duration is then **1 ms**. This edge rule is derived from the observed machine instruction, not a claim about portable C++ casts, other architectures, or an execution of the original with unmasked exceptions.

Corrected `src/lua_api.lua` to apply that conversion before clamping, using `math.modf` for truncation and explicit overflow/non-finite handling. The earlier direct floating clamp returned **65535 ms** for `2147483.648` seconds; the recovered rule returns **1 ms**. Ordinary `0.1239` seconds gives **123 ms**, negative/zero gives **1 ms**, and an in-range large int32 duration still caps at **65535 ms**. Regression `lua_duration_uses_native_integer_conversion_before_clamping` first failed at the overflow case (65535 versus 1) and passed after correction. It also covers both infinities, NaN and the last representable int32-millisecond boundary. Existing broader compatibility limits below still apply. Fresh validation after this correction: `cargo test --offline --locked` passed all **64 tests**; `cargo build --offline --locked` and `cargo fmt --check` passed. Craft Anything `rebuild_check` passed build, desktop launch/screenshot (938×990), and mechanics (17 entries, 16 sourced and one estimated).

## Fresh native audit — frame commands (2026-11-02)

Reopened the original main ELF with `core_target_open` in Deep mode, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Completed **eight sequential `native_function_pseudocode` calls**, waiting for and reading each result. Ghidra could not start; these are completed built-in fallback reads with register-level and control-flow limitations. Also read all 334 assembly instructions of the command to confirm the insertion argument omitted from the fallback's printed call.

Searched names, settings and messages with `native_strings_find`: `newFrame|newEmptyFrame|pixel_connectivity|commands.NewFrame_`, all 14 matches returned (`ev_071a16808d5cdd7ddd9ba32873391dea05603749a402b4c0935f5e58c9f6fd42`). Followed references for `newFrame` at `0xf41383` to registration-table locations `0x62550` / `0x1cbd290` (`ev_2298c06ae0b7fb314e49d20c350802667997051c748883e93d8d57e623e9e3ad`); the registration references alone do not identify executable code. Followed message `commands.NewFrame_` at `0xf43629` to `0x45d4ec` inside `NewFrameCommand::onExecute` (`ev_3bf55c6d7b194e92be2949a7a4f0d063baf029b55520dc2e35cc5da411375566`), and setting `pixel_connectivity` at `0xf38b5e` to `0x3769e3` inside the preference constructor (`ev_90cd14c274c97c7dc67174410855b57ad458062262a27e9928cf28fa5fd06993`). Followed resolved calls from the command to the frame APIs and copy operation. Previously resolved scripting wrappers were reread by address for the implementation comparison.

| Completed function read | Address | Rule / evidence |
| --- | --- | --- |
| `NewFrameCommand::onExecute` | `0x45cfa0` | Copy-frame and empty-frame branches pass **p = active zero-based index + 1** to their APIs. Assembly `0x45d07a..0x45d08b` and `0x45d1ed..0x45d1fe` explicitly load then increment `edx`. Pseudocode `ev_efc840d0c26554fa6f0ecc5eabba86e95a0e98e5a86963f7ec2ef3bef41d83ba`; full assembly `ev_fdabe0d53fa3a2ede74a2c9fdca77816e2e9b4dad3e8000e8af3aa8eb55d415b`. |
| `DocApi::addFrame` | `0x19f0b0` | Calls `copyFrame(sprite,p-1,p,0,1)`: source is the active frame, destination immediately after it. `ev_d599b298fdc5de6d02568eef5d323d34364a656b68fec2eb28de6500fb84bdaf`. |
| `DocApi::addEmptyFrame` | `0x19f250` | Executes AddFrame at p, then `adjustTags(sprite,p,1,0,1)`. `ev_7b33f23d34eaf610723a49480d7acc22162544085e91175322ddd84df943f8b5`. |
| `ToolPref::Floodfill` | `0x376670` | `pixel_connectivity` default remains enum 0 (four-neighbor mapping established in earlier audit). `ev_8c3ff38ff013d3407a8e58ce02489942a3c07b12a31fe73d2f7b0332a1d7b4a2`. |
| `DocApi::copyFrame` | `0x19f0d0` | Constructs copy operation, copies layer cels and adjusts tags. Fallback omits carry-dependent placement; earlier full assembly resolves it. `ev_c743cea957d1e245c2dce5260fbf18d16188a2f0f5e4b6daa717faf9408bfd6b`. |
| Lua `Sprite_newFrame` | `0x3f3b10` | Explicit source argument uses the insert-after path; no argument uses append. Explicit return handle is the source slot, so command code must not use that return value to select the inserted slot. `ev_dbeac66fc7b9fcd34a49fa0a3c17fa0d2f2d5c117c67d010b05c74cda81b819e`; register details established by earlier full assembly below. |
| Lua `Sprite_newEmptyFrame` | `0x3f3d80` | Passes requested insertion slot to `addEmptyFrame`; no argument appends. `ev_f76a8200024c4083d3d747fc0ff2b15ad0930a56d6a807656f47ec70abb5c383`; omitted moves established by earlier full assembly below. |
| `cmd::CopyFrame::onExecute` | `0x40d680` | Reads source duration, inserts destination, sets destination duration to source duration. `ev_5e1b9205922afe7910825763161fa0a9ef80f7d7d4600564730493f8f7657485`. |

Corrected `src/lua_api.lua`: `app.command.NewFrame()` and `NewEmptyFrame()` previously delegated to no-argument sprite methods and always appended. They now insert after the active frame. With one-based active frame f, copy uses `newFrame(f)` and empty uses `newEmptyFrame(f+1)`. PixelCraft retains its existing convention of selecting the inserted frame; exact original selection behavior through transaction notifications is not established here. Command parameters, linked-cel modes, background fill and multi-frame range selections remain outside this supported subset.

Regression `lua_frame_commands_insert_after_active_frame` reproduced the copy command's incorrect timing `[110,220,330,330]` instead of `[110,220,220,330]` from active frame 2. It checks both commands, preserved following artwork, copy versus empty pixels, and subsequent drawing into the inserted slot. It passed after the correction. Direct sprite methods retain their existing no-argument append behavior. Fresh validation: `cargo test --offline --locked` passed all **63 tests**; `cargo build --offline --locked` and `cargo fmt --check` passed. `rebuild_check` passed build, desktop launch/screenshot (942×1005), and mechanics (16 entries, 15 sourced and one estimated). The captured editor window was visually inspected.

## Latest native audit — empty-frame insertion (2026-11-02)

Fresh verification after this correction: `cargo test --offline --locked` passed **62 tests**; `cargo build --offline --locked` and `cargo fmt --check` passed. `rebuild_check` passed build, desktop launch/screenshot (942×1005), and mechanics (15 entries, 14 sourced and one estimated). The captured editor window was visually inspected.

Reopened the original main program `Aseprite 1.3.18.6 reference executable` using `core_target_open` in Deep mode. SHA-256: `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Completed and read **seven successful, sequential `native_function_pseudocode` calls**, awaiting each result. Ghidra could not start; the tool returned completed built-in fallback decompilation. Full assembly was also read where fallback omitted register assignments or conditional moves. An initial unqualified `Sprite_newEmptyFrame` lookup failed; symbol search resolved the actual function before its successful read.

`native_strings_find` searched API names, settings and messages (`newEmptyFrame|adjust.*tag|tag.*adjust|pixel_connectivity|commands.NewFrame_`), returning all nine matches, evidence `ev_f15bd130dda6f14d011db7396e3ff424eaf473895b5d930898ea12303bd94174`. Followed references from API name `0xf4138c` to registration-table locations `0x62580` and `0x1cbd2a0` (`ev_a1b3c39ebcd05391d6d0e9198d95e23e817d63d375347c9467f9d3a5f565caf0`); function-name search resolved the wrapper (`ev_7c0ade5850c6c7f661305856b4dfc497ec9c38664d35217add1dc2c2b7620627`). Followed message `commands.NewFrame_` at `0xf43629` to `0x45d4ec` in the command (`ev_3bf55c6d7b194e92be2949a7a4f0d063baf029b55520dc2e35cc5da411375566`), and `pixel_connectivity` at `0xf38b5e` to `0x3769e3` in its preference constructor (`ev_9514a6fed3f59c2e880c74f6078b95d543a81f27610065e9f83f91e98080d73b`). Followed calls from the wrapper through `addEmptyFrame`, `AddFrame::onExecute`, `Sprite::addFrame`, and `adjustTags`.

| Completed function read | Address | Recovered rule / evidence |
| --- | --- | --- |
| `NewFrameCommand::onExecute` | `0x45cfa0` | Empty-frame command calls `addEmptyFrame` at active index+1. Copy and empty paths differ. `ev_efc840d0c26554fa6f0ecc5eabba86e95a0e98e5a86963f7ec2ef3bef41d83ba`. |
| Lua `Sprite_newEmptyFrame` | `0x3f3d80` | Optional argument is converted to a zero-based frame index and passed unchanged to insertion and returned handle; omitted argument uses old frame count. Thus one-based `newEmptyFrame(f)` inserts **before f** and returns the inserted frame; omission appends. `ev_f76a8200024c4083d3d747fc0ff2b15ad0930a56d6a807656f47ec70abb5c383`; complete assembly `ev_a1000165a7d86428f85993e0d077f2b0abb2ede5f626ae3c93d41927c00c5ead`. |
| `DocApi::addEmptyFrame` | `0x19f250` | Executes AddFrame then `adjustTags(sprite,p,1,0,1)`. `ev_7b33f23d34eaf610723a49480d7acc22162544085e91175322ddd84df943f8b5`. |
| `cmd::AddFrame::onExecute` | `0x406d10` | Calls `Sprite::addFrame`; normal image layers get no copied cel. Background layer has a separate image filled with document background color. `ev_15c1ac27cf52a277a17e06dd402f400f83cc5270210e571b34601f1aba10c240`. |
| `Sprite::addFrame` | `0x61a140` | Grow timing array using old last duration, shift durations backward into following slots, and displace cels from insertion index. For valid existing durations, inserted duration equals the preceding frame's, or old first duration when inserting at zero. `ev_d9d069ecf6e53358e8d778aef660e719365a7bd3934c1ffa5ee3b0059e5b7c4d`; full assembly `ev_ccbfe1c1107895d2dcfc312e45fcb2ee98e70c49789375c1ffce13fe3e168956` resolves omitted conditional moves. |
| `DocApi::adjustTags` | `0x19f2d0` | For insertion mode 1 and zero-based slot p: `from += (p <= from)`; `to += (p <= to+1)`. This extends a tag when inserting immediately after its end. `ev_fcac7798cb5943d0f5803eadde1dc660820777919e18f7d9515257b002baf5d3`; full assembly `ev_1051621d6cfd1aa3e22ad12d52e81697e0607cd19f97fdc0e7df70f62f2ec18e`. |
| `ToolPref::Floodfill` | `0x376670` | `pixel_connectivity` default enum 0; installed enum mapping identifies four neighbors. `ev_8c3ff38ff013d3407a8e58ce02489942a3c07b12a31fe73d2f7b0332a1d7b4a2`. |

Changed `src/lua_api.lua`: `newEmptyFrame(f)` previously ignored its argument and called the copy-frame append operation, clearing copied pixels afterward. It now inserts empty pixel/cel/tile data at f, shifts following frames, inherits the preceding duration, adjusts tags with the recovered mode-1 inequalities, and returns handle f. No-argument insertion still appends. Parallel metadata and palette handling follow the project's preservation policy. The distinct `newFrame(f)` copy/return behavior documented below remains unchanged.

Regression `lua_empty_frame_inserts_at_requested_slot` failed before the fix with `empty insertion slot`. It exercises numeric and frame-handle arguments, timing `[110,220,330]` → `[110,110,220,330]`, empty pixels with preserved following artwork, insertion at the first frame, and omitted-argument append. Background-color filling, explicit nil/invalid arguments, absent-cel identity, and exact palette semantics are not established as equivalent by this correction. Other real values and formulas from earlier completed native reads remain documented below; historical unresolved tag-insertion statements are superseded for insertion mode 1 by this audit.

This is an independent Rust + eframe/egui implementation of the core pixel-art editing workflow. No original code, art, fonts, icons or logos are copied. The sample garden artwork is original and generated by this project. The project uses its dedicated Craft Anything app/ workspace, as requested.

## Latest executable audit — frame insertion

Reopened the original main ELF with `core_target_open` in Deep mode on 2026-11-02 (SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`). Completed and read **15 sequential `native_function_pseudocode` results**, awaiting each before proceeding. Ghidra could not start; every read completed using the built-in fallback. Assembly resolves the omitted register moves and carry-dependent instructions below. These are recovered rules for the supported subset, not a whole-app equivalence claim.

`native_strings_find` searched names, settings and messages (`newFrame|newEmptyFrame|tx.set_frame_|default_brush_size|pixel_connectivity|commands.NewFrame_`): all 16 matches returned, evidence `ev_8dff674b658af0e478c8ad5c272a9e13e81624272c9dcd0a637c743b5699b175`. Followed `newFrame` at `0xf41383` to its registration-table references (`ev_48738b91186a408532e0c3dd2896dd1daae0600d48c70d1ee5a93684af6d8570`); `tx.set_frame_` at `0xf401c6` to `0x3ce49d` in the setter (`ev_fdaeac98e8e0b0f835c01659e1c9dcf29e976e3aa6efeffc910c39701659b52c`); command message at `0xf43629` to `0x45d4ec` (`ev_a29e4a01d552f404b8e96d5cb429226a8e3fdca138deffc195f71059d6f17aa9`); and `pixel_connectivity` at `0xf38b5e` to its preference constructor (`ev_94cfc7820a0d713be3f3d9a8f7501baccae50672b5b4a96db2b97d1930bc971f`). Followed calls and exact symbol references from the frame wrapper through `DocApi::addFrame`, `copyFrame` and `cmd::CopyFrame::onExecute`.

### New correction used by the build

For an existing one-based Lua frame number `f` (or frame handle), `sprite:newFrame(f)` inserts a copy into slot **f+1**, shifting later frames. The explicit-argument path returns a handle for **f**, not the newly inserted slot. With no argument, it appends a copy of the last frame and returns the appended slot. This unusual explicit return rule comes directly from the executable, rather than an API assumption:

- `Sprite_newFrame`, **0x3f3b10**, pseudocode `ev_dbeac66fc7b9fcd34a49fa0a3c17fa0d2f2d5c117c67d010b05c74cda81b819e`; full assembly `ev_346a85fed2cc064370861c1293a2e3b47ee25367f8710f410aa65a173b902e52`. `r15d` holds source index; `ebp=source+1` is passed to `addFrame`; `r15d` is subsequently passed to `push_sprite_frame`. With no argument both start at the old frame count.
- `get_frame_number_from_arg`, **0x3ce270**, `ev_b32fbc0de647bef317f4c96d4183ebe37c7c5955193d3562b9343f56f130eb1e`: frame handles store zero-based indexes; numeric arguments subtract one.
- `push_sprite_frame`, **0x3ce200**, `ev_4278d9065e26f445cd14f96c9a4b71b9dc09ae9ad41113f8d2128e3a0dd73b04`; assembly `ev_3c0069bc2962fa5e57f712328245268f3e43d590802efd58d0f1df6e0a29cf2a` confirms it stores the passed index unchanged.
- `DocApi::addFrame`, **0x19f0b0**, `ev_d599b298fdc5de6d02568eef5d323d34364a656b68fec2eb28de6500fb84bdaf`: delegates to `copyFrame(sprite,n-1,n,0,1)`.
- `DocApi::copyFrame`, **0x19f0d0**, `ev_c743cea957d1e245c2dce5260fbf18d16188a2f0f5e4b6daa717faf9408bfd6b`; assembly `ev_5ba1c0c35a2adaf09e9d664b6eae3ab7b89ac3daa036f7e29aff5374cd5a5070` establishes that place=0 leaves the destination at n (`cmp`/`sbb`, omitted by fallback). It copies cels across layers and calls tag adjustment.
- `cmd::CopyFrame::onExecute`, **0x40d680**, `ev_5e1b9205922afe7910825763161fa0a9ef80f7d7d4600564730493f8f7657485`: reads source frame duration, inserts the destination frame and sets its duration.

Changed `src/lua_api.lua`: it previously appended every explicit copy and returned the last slot. It now inserts timing, pixels, cel metadata and tile data at f+1 and returns f for an explicit existing-frame argument. Parallel palette and metadata arrays stay aligned; tag movement follows PixelCraft's existing model insertion policy (exact original tag adjustment is still unverified). Resource and invalid-frame checks remain project safeguards.

Regression `lua_new_frame_inserts_after_argument_and_returns_source_slot` failed before the correction with `returned source slot`, then passed. Both numeric and frame-handle arguments are exercised. Starting durations `[110,220,330]`, copying frame 1 and assigning 444 ms through the returned handle produces `[444,110,220,330]`. Editing the inserted copy leaves original and later-frame pixels intact.

### Other completed reads

| Function | Address | Recovered rule / evidence |
| --- | --- | --- |
| `NewFrameCommand::onExecute` | `0x45cfa0` | Separate empty/copy/linked paths in a transaction; `ev_efc840d0c26554fa6f0ecc5eabba86e95a0e98e5a86963f7ec2ef3bef41d83ba`. |
| Lua `Frame_set_duration` | `0x3ce440` | Float multiply then truncating integer conversion; `ev_3f22c15620330775cce907dab0a4ff7783531be37e91569f301d264ec7be2c71`. |
| `Sprite::setFrameDuration` | `0x61a420` | Valid frames only, 1..65535 ms; `ev_306fda5d2ea134eb34ae1bdc260478f8ea5224238ba81714adc96eff875ea5e6`; earlier complete assembly E4 supplies conditional-move bounds. |
| `ToolPref::Floodfill` | `0x376670` | Default connectivity enum 0; installed enum mapping gives four neighbors; `ev_8c3ff38ff013d3407a8e58ce02489942a3c07b12a31fe73d2f7b0332a1d7b4a2`. |
| `Sprite::Sprite` | `0x617e40` | One initial frame, 100 ms; `ev_5293b37d970497833b57985911b6bce97c564b236dba27819fbf5a45298f1f54`. Also establishes frame-count field at +0x98 used by scripting wrapper. |
| `rgba_blender_normal` | `0x5e81e0` | `M(a,b): t=a*b+128; (t+(t>>8))>>8`; s=M(src alpha,opacity); A=s+d-M(s,d); C=dst+trunc((src-dst)*s/A), transparent branches as below; `ev_50358e5f77cd676585cc0771461863366aac18f4c445a730cc44c0963de7e730`. |
| `rgba_to_graya_using_luma` | `0x5f7170` | floor((2126R+7152G+722B)/10000), alpha preserved; `ev_b3bd20c7de1394f17ad16847d892a2a329bcbbe24c08c58abb94cb04374eece9`. |
| `ContextBar::BrushSizeField` | `0x253050` | UI input range 1..64 px; `ev_5598869494a7abf4f39d06f29193b9deac86fc01207474f0ccd7a81a49d45115`. |
| `algo_line_continuous` | `0x5cb800` | Integer error stepping with endpoints, recurrence below; `ev_67a87e62bf30b0c8227a6a7d910984e565c70dbb19fe8fb9e3d224c831c46f90`. |

Fresh verification: `cargo test --offline --locked` passed all **61 tests**; `cargo build --offline --locked` and `cargo fmt --check` passed. Craft Anything `rebuild_check` passed build, desktop launch/screenshot (942×1005), and mechanics (14 entries, 13 sourced and one estimated).

Explicit nil/out-of-range scripting frame arguments, exact native tag/palette movement, advanced blend formulas, ellipse masks and complete ecosystem compatibility remain outside verified parity. The earlier broad statement that scripting frame insertion is wholly unresolved is superseded for valid existing-frame arguments by this audit.

## Current native re-audit — 2026-11-02

Opened `Aseprite 1.3.18.6 reference executable` again with `core_target_open`; SHA-256 is `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. Awaited **12 separate `native_function_pseudocode` calls**, sequentially, before making the correction below. Earlier audit sections are retained as historical evidence.

### Search → references → completed reads

- `native_strings_find` searched names, settings and messages using `commands.NewFrame_|pixel_connectivity|default_brush_size|duration|grayscale|opacity`: all 130 matches returned (`ev_702af70a56ae5c75b4d59da511e291dfdb4a328d4dfb6414bb312b98a33bdd50`). Followed `commands.NewFrame_` at `0xf43629` with `native_xrefs_list`, finding the command at `0x45cfa0` (`ev_3e6fec8d3a7f2e5cc73639baede946684f5d92ec9ae55386c9dfa1024ca3b097`).
- Duration message `tx.set_frame_` at `0xf401c6` points to the Lua setter at `0x3ce440` (`ev_bbde977b209b3da9ae25d018cdaa06f07d1946f4ad86610bfeb1379a36c3cd2c`). Function-name search resolved scripting duration, frame insertion and image drawing (`ev_451a5332be6cf5f8b3e41c735be9cced1f74f58236a497f611a59f5343b19ff9`). An attempted mangled name using `frame_t` did not resolve; the search found the actual `int` signature instead. The unrelated `duration` string at `0xf401d3` belongs to open-sequence/App-tip code, not the frame setter.
- `drawImage` string `0xf40232` has static table references (`ev_e528c013885fe981bc6fd561db4b6ec2f093525185490f1ecf961cf01e115e03`). Resolved `Image_drawImage` by function-name search, then followed its named call to `doc::blend_image` (`ev_79717fb7c15f4df8f319b00297410e643f129e04d77ec801629af8246351e6ea`), the RGBA template (`ev_92b19b1aab2f126f07f105399b83fb5c46736dd0cbad6aabbf38843041d39039`), and `get_rgba_blender` (`ev_292666c272415e687f65c71bf39f95519419d2d4d57e8f37223920ae52ac9cf3`). This establishes an image-drawing call chain, rather than assuming that a similarly named blend helper is used.

| Completed function read | Entry | Result / evidence |
| --- | --- | --- |
| `Sprite::setFrameDuration` | `0x61a420` | Valid frame only; clamp 1..65535 ms. Pseudocode drops conditional moves; earlier complete assembly remains decisive (E4). |
| Lua `Frame_set_duration` | `0x3ce440` | Transactional `SetFrameDuration`; floating value multiplied then converted with `cvttsd2si`, i.e. truncation toward zero. `ev_3f22c15620330775cce907dab0a4ff7783531be37e91569f301d264ec7be2c71`. Positive fractional seconds already truncate in the build; this read alone does not decode the multiply constant. |
| Lua `Sprite_newFrame` | `0x3f3b10` | Calls `DocApi::addFrame`; explicit frame argument influences insertion. `ev_dbeac66fc7b9fcd34a49fa0a3c17fa0d2f2d5c117c67d010b05c74cda81b819e`. Register recovery is incomplete; exact return-handle and default-insertion semantics are unresolved. The build's optional-frame scripting path has not been established as native-equivalent. |
| Lua `Image_drawImage` | `0x3d2bb0` | Calls `doc::blend_image` for detached images and a temporary-image/transaction path for cel images. `ev_ff73745cee8b773b1314eae209eab59fc0e5e7600fb80054f6c10d80876d1063`. |
| `doc::blend_image` | `0x5efb30` | Clips source/destination, dispatches by color type; RGBA→RGBA goes to `0x5eff40`. `ev_28538c4a0e16bd8f6fbda850c9bb9df7373a9f28220187a84821f8b0d463d69c`. |
| `blend_image_templ<RgbTraits,RgbTraits>` | `0x5eff40` | Gets blender with second argument true; normal path skips source pixels equal to image mask color before calling blender. `ev_db08cb397bdee123a1f98e80fbf399af866a7b5db87f61df1f18268fac676296`. PixelCraft RGBA images use packed zero as that mask. |
| `get_rgba_blender` | `0x5ef7d0` | Returns normal helper at `0x5e81e0`; advanced modes select legacy or `_n` variants with `test sil; cmovne`. Pseudocode `ev_725d9adf5c89add4ec405b569461dfa7ec7977993e166cc488c5cadc61fc94ee`; complete assembly `ev_c8f5bade2b73099403f541dcd91b048c374649405f88b11a6d49a5031e731c96`. This clarifies selection for image drawing, not all renderer settings. |
| `rgba_blender_normal` | `0x5e81e0` | Byte products and signed channel division; re-read pseudocode and all 87 instructions (E12). Formula below. |
| `Sprite::Sprite` | `0x617e40` | One frame, 100 ms initial duration; re-read E3. |
| `ContextBar::BrushSizeField` | `0x253050` | `IntEntry(1,64,null)`, px suffix; re-read E7. |
| `rgba_to_graya_using_luma` | `0x5f7170` | Integer weighted sum, no rounding bias; re-read `ev_b3bd20c7de1394f17ad16847d892a2a329bcbbe24c08c58abb94cb04374eece9`. Formula remains floor((2126R+7152G+722B)/10000), alpha preserved. |
| `DocApi::addFrame` | `0x19f0b0` | `copyFrame(sprite,n-1,n,0,1)`; re-read E11. |

### Correction applied to the current build

`app/src/lua_api.lua` previously used floating source-over with nearest rounding in `Image:drawImage`, independently of the already-correct Rust normal compositor. It now uses the recovered integer rule for supported RGBA normal image drawing:

```text
M(a,b): t=a*b+128; return (t+(t>>8))>>8
skip packed-zero source mask pixels
s=M(source.alpha,opacity)
if destination.alpha==0: return [source.rgb,s]
if source.alpha==0: return destination
A=s+destination.alpha-M(s,destination.alpha)
C=destination.C+trunc_toward_zero((source.C-destination.C)*s/A)
```

For destination `[10,20,30,100]`, source `[200,100,0,128]`, opacity 172, the previous Lua output was **[118,65,13,152]**; the recovered rule gives **[117,65,14,152]**. Hidden RGB in nonzero packed source colors is retained when both alphas are zero; packed-zero mask pixels leave the destination unchanged. `newCel` also consumes this drawing helper.

Regression `lua_draw_image_uses_native_integer_normal_blend` failed on the old arithmetic, then passed after the correction. Its five literal cases cover the translucent rounding difference, both transparent branch orders, zero opacity and mask skipping, through the embedded Lua runtime and imported document pixels.

**Verification in this re-audit:** `cargo test --offline --locked` passed **60 tests**, `cargo build --offline --locked` and `cargo fmt --check` passed. Craft Anything `rebuild_check` also passed the build, launch/screenshot (938×990), and mechanics checks (13 entries: 12 sourced, one estimated).

**Limits:** all 12 pseudocode results report `deep_engine_unavailable`: Ghidra failed to start, and the completed built-in fallback results were read instead. Assembly supports the integer arithmetic and dispatcher correction. This is a focused audit of the functions listed, not proof that every feature matches the original. Advanced blend math, non-RGBA scripting images, overlapping self-draw, exact ellipse masks, scripting frame insertion and full tag playback remain unverified.

## Deep-mode follow-up — 2026-11-02

Reopened the original main executable with `core_target_open` in this session; its SHA-256 still matches the original below. Ran `native_strings_find` for command names, settings and messages, followed string and symbol references, and awaited **16 individual `native_function_pseudocode` results**, sequentially. This audit extends the earlier evidence rather than assuming its conclusions still match the expanded build.

**New correction used by the build:** `doc::rgba_to_graya_using_luma` at **`0x5f7170`** computes:

```text
N = 2126*R + 7152*G + 722*B
Y = floor(N / 10000)
output = [Y, Y, Y, input_alpha]
```

The machine code multiplies N by `0x1a36e3` then shifts right 34 bits, without a rounding bias. Exhaustive arithmetic comparison for every integer N in `0..=2550000` confirms equivalence to the division above throughout the byte RGB input range. `model::convert` previously added 5000, incorrectly rounding `[200,100,50,173]` to `[118,118,118,173]`; it now yields **`[117,117,117,173]`**. Both existing-sprite conversion and grayscale drawing use the corrected helper. Alpha, including zero, is preserved. Two regression tests first failed with the old rounded output, then passed after this correction.

### Fresh reference trail and reads

- Search `commands.NewFrame_|pixel_connectivity|default_brush_size|duration|grayscale|Ping-pong`: `ev_a5004e1837c3846a56ef4206f2eb1bc4469bda040efbc204b04d642090c28cc1` (focused first page, 65/93 matches). Additional complete search `to_gray|new_blend|luma|commands.NewFrame_`: `ev_8d81cee5896357da41cbff317cc89942e082039abda2e69f049330697aef243a`.
- Grayscale command label `0xf41e4d` → references in `ChangePixelFormatCommand::onExecute`, entry **`0x4265d0`** (`ev_b749c6bd0b23bd2fab5882e9f839281ebf1bd75777b524e062036f1365771a1d`). Read its pseudocode (`ev_241dd5abe29f088fa6e26b6ad208f3852efec159fb45bce14909b9dd2fe5e5e2`) and job callback **`0x4293c0`** (`ev_596cdbde12440afc2eae4397e389dd9b9a9a3ff5a92b3702efb4597e6c442b3f`); callback constructs `SetPixelFormat` and selects a grayscale conversion function from a table. The UI offers multiple grayscale algorithms; this correction implements luma, not every algorithm.
- Native conversion call references connect `SetPixelFormat::convertImage`'s call at `0x524be3` to **`render::convert_pixel_format`, `0x5afc10`** (`ev_5d0504684b76cae34617c98b2930b9709d88887c9f7b86308536003aec41a807`). Read conversion pseudocode (`ev_a949887b64ddfff4620b3b3ddd57f170a493f2a00c2b14316e2efe757e94d047`): for RGB/indexed → grayscale with a null conversion callback it defaults to `rgba_to_graya_using_luma`. Followed that symbol's references (`ev_8914993f719080b9b9d91d063c21dc548631ff907598ce548865888c4265dc5a`), then read **`0x5f7170`** pseudocode (`ev_b3bd20c7de1394f17ad16847d892a2a329bcbbe24c08c58abb94cb04374eece9`) and complete assembly (`ev_e86fd3d67feb70ea03f414eac482181d11c32b8605b6293211b326b621c63028`). These establish the formula and rounding correction.
- Re-followed `commands.NewFrame_`, `pixel_connectivity` and `default_brush_size` references. Re-read **`NewFrameCommand::onExecute`, `0x45cfa0`**, and **`DocApi::addFrame`, `0x19f0b0`** (E11 below). The latter still delegates to `copyFrame(sprite,n-1,n,0,1)`; distinct empty/copy/linked paths remain visible in the command.
- Re-read **`Sprite::Sprite`, `0x617e40`** (E3), **`Sprite::setFrameDuration`, `0x61a420`** (E4), **`ContextBar::BrushSizeField`, `0x253050`** (E7), **`rgba_blender_normal`, `0x5e81e0`** (E12), and **`algo_line_continuous`, `0x5cb800`** (E9). Existing build values remain 100 ms, 1..65535 ms and 1..64 px, with the integer normal-blend and line rules below.
- Re-read preference constructors **`GlobalPref::NewFile`, `0x36b560`** (`ev_baa5de5327fe54659d448ddbfd1ab98a20664a76b41de245ad6abe31350283d2`), **`ToolPref::Brush`, `0x375c10`** (`ev_b1215cc04e895bf8efd5853d2d65d833c886f07196667d676657258d14e3bf1b`), and **`ToolPref::Floodfill`, `0x376670`** (`ev_8c3ff38ff013d3407a8e58ce02489942a3c07b12a31fe73d2f7b0332a1d7b4a2`). Observed 32×32 defaults, brush size 1 and connectivity enum 0 agree with the earlier audit.
- Followed multiply-blend symbol references (`ev_5a91c3552a5e99a5f36f23d2457d3f4123294d266d1d3492447db07f96a0c724`) and read **`rgba_blender_multiply`, `0x5e84b0`** (`ev_026223eaa242f26e6c7302116bb1bc416d292cd379be6ccea4464b1275e4dfd3`) and **`rgba_blender_multiply_n`, `0x5e9a20`** (`ev_ac5a182f1a057fcd9e60bdcf1b7b4287a363760965841ab51d5f8a4cfbfb204b`). The legacy routine multiplies byte channels with the M rounding below and tail-calls normal blend. The newer routine adds alpha-dependent interpolation. The fallback drops register/conditional-move details in that routine; its exact formula and which variant the renderer selects are unresolved here. The build's advanced floating-point blends remain independent approximations; these reads do not establish native parity for them.

**Engine boundary:** Ghidra again failed to start analysis; every pseudocode result reports `deep_engine_unavailable` and the built-in lifter. We read those completed results, not queued jobs. Grayscale luma has no untranslated instructions and is additionally verified against its 15-instruction assembly. Lossy duration/line/normal-blend pseudocode still requires the earlier assembly evidence below. This is a focused audit of recovered editing mechanics, not decompilation or parity verification of every feature in the expanded editor. Native ellipse masks, advanced blend selection/rounding, indexed best-fit criteria and complete tag playback remain open questions.

**Fresh verification:** `cargo test --offline` passed all **59 tests**; `cargo fmt --check` and `cargo build --offline` passed. Craft Anything `rebuild_check` passed build, launch/screenshot (942×1005) and mechanics (12 entries, 11 sourced and one explicitly estimated). These checks establish a working rebuilt editor and regression coverage, not whole-application equivalence.

## Native audit — 2026-11-01

Reference application: Aseprite 1.3.18.6, ELF x86-64, SHA-256 `01841505e2dbef652d51380bcac03aba1188b9462f92ea3bac6d87630a7234e6`. This reference was used during development; PixelCraft does not require its executable, installation files or settings.
Opened with `core_target_open` in Deep mode. Every function listed below was read with `native_function_pseudocode`, awaiting its completed result. **Engine limitation:** Ghidra could not complete analysis; these results came from Craft Anything's built-in decompiler. Its register-level pseudocode drops some conditional moves and leaves SIMD and signed division as assembly. Duration, line stepping and normal blending were therefore also checked with `native_function_disassemble`. This is completed decompilation with partial type/control-flow recovery, not a successful Ghidra analysis or proof of whole-app parity.

### Search and reference trail

`native_strings_find` searched command names, settings and messages (E1, E2). `native_xrefs_list` followed `commands.NewFrame_` at `0xf43629` to `NewFrameCommand::onExecute` at `0x45cfa0`; outgoing named calls led to `DocApi::addFrame` at `0x19f0b0`. The `default_brush_size` string at `0xf37381` references `ToolBox::loadTools` (not used as proof of a numeric limit). The `size` setting at `0xf7212a` references the brush-preference constructor at `0x375c10`; `pixel_connectivity` at `0xf38b5e` references the flood-fill preference constructor at `0x376670`; `width` and `height` reference the new-file preference constructor at `0x36b560`.

Named references additionally connect `Brush::setSize` to `Brush::regenerate` at `0x5f14c0`, and the context-bar constructor's call at `0x24c179` to `BrushSizeField` at `0x253050`. Incoming references to normal RGBA blending include transparent ink, image blending and the blend-mode dispatcher. Symbol addresses were resolved from the original binary, not guessed from the Rust build. Strings also expose the export alerts `alerts.file_format_20ms_min_duration` and `alerts.file_format_10ms_duration_precision`; their names alone do not establish the original GIF encoder's complete policy.

### Recovered values and rules

| Function (entry address) | Native observation / recovered rule | Build use |
| --- | --- | --- |
| `doc::Sprite::Sprite` (`0x617e40`) | Frame count initialized to **1**, duration vector's first entry to `0x64` = **100 ms**. E3. | `Document::new`, `DEFAULT_DURATION`. |
| `doc::Sprite::setFrameDuration` (`0x61a420`) | Ignore frame indexes outside `[0, frame_count)`. For valid indexes, `duration = clamp(requested_ms, 1, 65535)`; signed `cmovge` and `cmovl` confirm the limits. E4. | Timeline input already limits to **1..65535 ms**; ASE import maps zero to 1. |
| `GlobalPref::NewFile` constructor (`0x36b560`) | Width and height each initialize their default/current integer pair to `0x20` = **32** (`0x2000000020`). E5. | New-sprite dialog starts at **32 × 32**. |
| `ToolPref::Brush` constructor (`0x375c10`) | Size default/current pair is `0x100000001`: **1 pixel**; type defaults to enum 0, angle to 0. E6. | Initial brush size **1**. |
| `ContextBar::BrushSizeField` constructor (`0x253050`) | Calls `ui::IntEntry(1, 0x40, null)` and sets `px` suffix: UI bounds **1..64 pixels**. This is an input-field limit, not proof of a global engine cap. E7. | Corrected the previous arbitrary 32 cap to **64**, through `MIN_BRUSH_SIZE` / `MAX_BRUSH_SIZE`. |
| `doc::Brush::setSize` (`0x5f29c8`), `regenerate` (`0x5f14c0`) | Setter stores size and tail-calls regeneration. Regeneration anchors the mask at negative half-dimensions; size 1 fills its single pixel. Round type 0 invokes `fill_ellipse` over `0..size-1`. E8. | Stamp anchor is `-(size / 2)` with size 1 covering one pixel. See ellipse limitation below. |
| `doc::algo_line_continuous` (`0x5cb800`) | Integer stepping: `dx=abs(x1-x0)`, `dy=abs(y1-y0)`, `err=dx-dy`; emit current point; `e2=2*err`; step x if `e2>=-dy` and y if `e2<=dx`, updating err by `-dy` / `+dx`. Each endpoint is included; native checks the relevant endpoint before each step. E9. | Existing integer line tool and pencil interpolation already implement this recurrence. |
| `ToolPref::Floodfill` constructor (`0x376670`), `FloodFillPointShape::transformPoint` (`0x21fbf0`) | Connectivity enum defaults to **0**; installed `pref.xml` maps enum 0 to **FOUR_CONNECTED**, i.e. 4 neighbors. The point-shape reads the seed pixel and passes mask, bounds and tool-loop options to native floodfill. E10. | Existing exact-color contiguous fill visits left/right/up/down. Other native options remain outside the build's subset. |
| `NewFrameCommand::onExecute` (`0x45cfa0`), `DocApi::addFrame` (`0x19f0b0`), `cmd::AddFrame::onExecute` (`0x406d10`), `doc::Sprite::addFrame` (`0x61a140`) | New-frame command has distinct empty/copy/linked-copy paths, inside a document transaction. `DocApi::addFrame(sprite,n)` calls `copyFrame(sprite,n-1,n,0,1)`. Sprite insertion grows timing data and shifts later frame entries/cels. E11. | Duplicate copies the previous selected frame's pixels and timing, inserting at `f+1`; empty frames contain transparent cels. Linked copies and special background layers are unsupported. Full selection/tag behavior is not recovered here. |
| `doc::rgba_blender_normal` (`0x5e81e0`) | Exact integer normal blend described below; signed `idiv` is decisive. E12. | Replaced the previous rational/nearest-rounding blend in `model::over`, used by layer composition and paste. |

### Exact normal RGBA blend

For byte values, define `M(a,b)`: `t=a*b+128; M=(t+(t>>8))>>8`.
Let `s=M(source_alpha,opacity)`, `d=destination_alpha`.

- If `d==0`, return source RGB with alpha `s`, even when `s==0`.
- Otherwise, if the original source alpha is zero, return the destination unchanged.
- Otherwise, output alpha is `A=s+d-M(s,d)`.
- Each output color channel is `destination + trunc_toward_zero((source-destination)*s/A)`.

The order matters: `[10,20,30,100]` under source `[200,100,0,128]` at opacity `172` gives `s=86`, `A=152`, result **[117,65,14,152]**. The old build incorrectly produced blue 13. Regression tests cover this case and both transparent branches. This establishes this specific normal-blend routine; advanced blend modes/color management remain unsupported.

### Evidence ledger

Evidence records reside under `investigation.cany/evidence/` (first two digest characters are the subdirectory). Related reference results are also retained there.

- E1 string search: `ev_8b326406c675df13dded9fddcdaca6460ab5be03252d2538d92845316bcd376d`.
- E2 focused settings/name search: `ev_8934592e54aeabaf87b592480cb6e438d8cf463a2d425dcfcf1bf1e1422c46f4`.
- E3 sprite constructor: `ev_5293b37d970497833b57985911b6bce97c564b236dba27819fbf5a45298f1f54`.
- E4 duration pseudocode: `ev_306fda5d2ea134eb34ae1bdc260478f8ea5224238ba81714adc96eff875ea5e6`; assembly: `ev_191ac27d385689c1e5621fa4eb0a424e4668ed37a2667562269756a622e1b04d`.
- E5 new-file defaults: `ev_0b33fe42bcb4cbfeca2e092fbfb90845652cf5cd31e40f3e5c8dc9d4b711b17c`.
- E6 brush defaults: `ev_84819a165a6528e2aacd600293a4c08d6cf235425805c93ccaeb89eb59a88bea`.
- E7 brush size field: `ev_5598869494a7abf4f39d06f29193b9deac86fc01207474f0ccd7a81a49d45115`.
- E8 size setter: `ev_53c98be3552312a6bc078af50f3a278356976353c86228eab025be9147e7981e`; regeneration: `ev_7a0b49410133dd68ad5ef9cd93a820734ef769678141573a445ab771de983274`.
- E9 line pseudocode: `ev_67a87e62bf30b0c8227a6a7d910984e565c70dbb19fe8fb9e3d224c831c46f90`; assembly: `ev_a2c360db53435d40e948815fe5b2f45602a2baac07177192a33848db6a4d2d22`.
- E10 fill preference: `ev_e00105a82bb9de216da266f3ab57434599f4918e6c3a6c72e64a61b4197919c9`; point-shape: `ev_478264573ee0d4d901af2e2e901cd0f82ec3c9ff262d3455af7abdf02aa685c5`.
- E11 new-frame command: `ev_efc840d0c26554fa6f0ecc5eabba86e95a0e98e5a86963f7ec2ef3bef41d83ba`; API: `ev_d599b298fdc5de6d02568eef5d323d34364a656b68fec2eb28de6500fb84bdaf`; command insertion: `ev_15c1ac27cf52a277a17e06dd402f400f83cc5270210e571b34601f1aba10c240`; sprite insertion: `ev_d9d069ecf6e53358e8d778aef660e719365a7bd3934c1ffa5ee3b0059e5b7c4d`.
- E12 normal blend pseudocode: `ev_50358e5f77cd676585cc0771461863366aac18f4c445a730cc44c0963de7e730`; assembly: `ev_821f8fa80d93b2ce55cfbe7f3c6809a9e49134f116f8f8af3a438d864fbec6fa`.

### Rasterization update

The current editor uses an integer symmetric ellipse rasterizer with degenerate-size handling, plus selectable round, square, diamond and line brushes. The original ellipse pseudocode did not establish all native masks; exact native parity is not claimed. Normal blend integer rounding remains covered by the original regression tests. Other blend modes follow standard source-over blend formulas.

## Interface evidence and choices

Installed extensions/aseprite-theme/theme.xml defines the light editor face #d3cbbe, editor background #655561, selection #7c909f and active color #ff5555. Our layout follows its menu/context bar, left palette, right tools and bottom timeline. Standard bundled egui fonts and independently drawn tool icons replace original assets. New canvases use the original installed pref.xml new_file defaults of 32x32 pixels. Demo dimensions (48x48), initial zoom and sidebar sizes are project usability choices, not original constants. Original runtime capture was refused by the tool's approval review; pixel-exact likeness is unverified.

## Current standalone editor

The November 2 standalone upgrade supersedes the old RGBA-only model and original-executable bridge. Native `.pixel` persistence stores all editor state; ASE is optional native interchange. Indexed/grayscale colors, groups, 19 blend modes, cel opacity/order, tags/ranges, tile layers, embedded tilesets, opaque metadata, ICC bytes, image pixels outside the canvas, layer UUID/flags, palette names and PNG ancillary metadata are supported. Detailed interchange boundaries and native Lua API are in README.md.

Limits are independent engineering choices: 16384 pixels per side, 64 million stored canvas pixels, 64 undo steps with a 192 MB allocation budget. Cel/index buffers share storage until edited; frame textures and occupied-cell indicators are cached. Timeline thumbnails are limited to the first 256 frames. Lua is bundled independently (MIT license), with resource budgets and transactional results. Installed extensions and settings belong solely to Pixel Workshop.

Standalone verification runs the actual binary inside an otherwise empty Bubblewrap filesystem with only basic Linux dynamic libraries, no original executable/settings/assets and no system Lua. It checks drawing, native save/open, PNG/GIF/sheet export, native plugin installation/execution and theme/palette/shortcut application, disable and uninstall. See checks/standalone.sh.
