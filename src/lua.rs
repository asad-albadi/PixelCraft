//! Lua is embedded as bytes in the executable, with no installed interpreter required.
use crate::model::Document;
use libloading::Library;
use std::{
    ffi::{CStr, c_char, c_int, c_void},
    path::PathBuf,
};
type State = *mut c_void;
type Alloc = unsafe extern "C" fn(*mut c_void, *mut c_void, usize, usize) -> *mut c_void;
unsafe extern "C" {
    fn realloc(p: *mut c_void, n: usize) -> *mut c_void;
    fn free(p: *mut c_void);
}
pub type HostCallback =
    Box<dyn FnMut(serde_json::Value) -> Result<serde_json::Value, String> + Send>;
struct HostContext {
    callback: HostCallback,
    string: unsafe extern "C" fn(State, c_int, *mut usize) -> *const c_char,
    push: unsafe extern "C" fn(State, *const c_char, usize) -> *const c_char,
}
thread_local! { static HOST: std::cell::Cell<*mut HostContext> = const { std::cell::Cell::new(std::ptr::null_mut()) }; }
unsafe extern "C" fn host_call(state: State) -> c_int {
    HOST.with(|slot| {
        let ptr = slot.get();
        if ptr.is_null() {
            return 0;
        }
        let ctx = unsafe { &mut *ptr };
        let mut len = 0;
        let input = unsafe { (ctx.string)(state, 1, &mut len) };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if input.is_null() {
                return Err("Host request must be a string".to_string());
            }
            let bytes = unsafe { std::slice::from_raw_parts(input.cast::<u8>(), len) };
            let value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
            (ctx.callback)(value)
        }))
        .unwrap_or_else(|_| Err("Extension host failed".into()));
        let value = result.unwrap_or_else(|e| serde_json::json!({"error":e}));
        let output = format!("return {}", literal(&value));
        unsafe {
            (ctx.push)(state, output.as_ptr().cast(), output.len());
        }
        1
    })
}
struct Budget {
    used: usize,
}
unsafe extern "C" fn allocate(
    ud: *mut c_void,
    p: *mut c_void,
    old: usize,
    new: usize,
) -> *mut c_void {
    let budget = unsafe { &mut *(ud as *mut Budget) };
    let old = if p.is_null() { 0 } else { old };
    if new == 0 {
        unsafe { free(p) };
        budget.used = budget.used.saturating_sub(old);
        return std::ptr::null_mut();
    }
    let total = budget.used.saturating_sub(old).saturating_add(new);
    if total > 256 * 1024 * 1024 {
        return std::ptr::null_mut();
    }
    let out = unsafe { realloc(p, new) };
    if !out.is_null() {
        budget.used = total;
    }
    out
}
struct RuntimeDirectory(PathBuf);
impl Drop for RuntimeDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Runtime {
    host: Option<Box<HostContext>>,
    library: Library,
    state: State,
    // Fields drop in declaration order: unload the DLL before deleting it on Windows.
    _directory: RuntimeDirectory,
    _budget: Box<Budget>,
}
impl Runtime {
    fn new(callback: Option<HostCallback>) -> Result<Self, String> {
        // Clock timestamps can repeat for concurrent calls on Windows.
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "pixelcraft-lua-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos(),
            NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let directory = RuntimeDirectory(directory);
        #[cfg(target_os = "windows")]
        let (name, bytes) = (
            "lua54.dll",
            include_bytes!("../runtime/lua54.dll").as_slice(),
        );
        #[cfg(target_os = "macos")]
        let (name, bytes) = (
            "liblua5.4.dylib",
            include_bytes!(env!("PIXELCRAFT_MACOS_LUA")).as_slice(),
        );
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let (name, bytes) = (
            "lua.so",
            include_bytes!("../runtime/liblua5.4.so").as_slice(),
        );
        let path = directory.0.join(name);
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        let library = unsafe { Library::new(&path) }.map_err(|e| e.to_string())?;
        let mut budget = Box::new(Budget { used: 0 });
        let state = unsafe {
            library
                .get::<unsafe extern "C" fn(Alloc, *mut c_void) -> State>(b"lua_newstate\0")
                .map_err(|e| e.to_string())?(
                allocate, (&mut *budget as *mut Budget).cast()
            )
        };
        if state.is_null() {
            return Err("Could not initialize embedded Lua".into());
        }
        unsafe {
            library
                .get::<unsafe extern "C" fn(State)>(b"luaL_openlibs\0")
                .map_err(|e| e.to_string())?(state)
        };
        let mut host = if let Some(callback) = callback {
            Some(Box::new(HostContext {
                callback,
                string: unsafe {
                    *library
                        .get::<unsafe extern "C" fn(State, c_int, *mut usize) -> *const c_char>(
                            b"lua_tolstring\0",
                        )
                        .map_err(|e| e.to_string())?
                },
                push: unsafe {
                    *library
                        .get::<unsafe extern "C" fn(State, *const c_char, usize) -> *const c_char>(
                            b"lua_pushlstring\0",
                        )
                        .map_err(|e| e.to_string())?
                },
            }))
        } else {
            None
        };
        if let Some(context) = host.as_mut() {
            HOST.with(|slot| slot.set(&mut **context));
            unsafe {
                library.get::<unsafe extern "C" fn(State,unsafe extern "C" fn(State)->c_int,c_int)>(b"lua_pushcclosure\0").map_err(|e|e.to_string())?(state,host_call,0);
                library
                    .get::<unsafe extern "C" fn(State, *const c_char)>(b"lua_setglobal\0")
                    .map_err(|e| e.to_string())?(state, c"_pw_host".as_ptr());
            }
        }
        Ok(Self {
            host,
            library,
            state,
            _directory: directory,
            _budget: budget,
        })
    }
    fn eval(&self, source: &str) -> Result<(), String> {
        self.load_chunk(source)?;
        self.call(0, 0)
    }

    fn load_chunk(&self, source: &str) -> Result<(), String> {
        unsafe {
            let load = self
                .library
                .get::<unsafe extern "C" fn(
                    State,
                    *const c_char,
                    usize,
                    *const c_char,
                    *const c_char,
                ) -> c_int>(b"luaL_loadbufferx\0")
                .map_err(|e| e.to_string())?;
            if load(
                self.state,
                source.as_ptr().cast(),
                source.len(),
                c"PixelCraft".as_ptr(),
                c"t".as_ptr(),
            ) != 0
            {
                return Err(self.take_string()?);
            }
            Ok(())
        }
    }

    fn take_string(&self) -> Result<String, String> {
        unsafe {
            let mut length = 0;
            let p = self
                .library
                .get::<unsafe extern "C" fn(State, c_int, *mut usize) -> *const c_char>(
                    b"lua_tolstring\0",
                )
                .map_err(|e| e.to_string())?(self.state, -1, &mut length);
            let value = if p.is_null() {
                Err("Lua returned no result".into())
            } else {
                Ok(
                    String::from_utf8_lossy(std::slice::from_raw_parts(p.cast::<u8>(), length))
                        .into_owned(),
                )
            };
            self.library
                .get::<unsafe extern "C" fn(State, c_int)>(b"lua_settop\0")
                .map_err(|e| e.to_string())?(self.state, -2);
            value
        }
    }

    fn global_string(&self, name: &CStr) -> Result<String, String> {
        unsafe {
            self.library
                .get::<unsafe extern "C" fn(State, *const c_char) -> c_int>(b"lua_getglobal\0")
                .map_err(|e| e.to_string())?(self.state, name.as_ptr());
        }
        self.take_string()
    }

    fn take_global_function(&self, name: &CStr) -> Result<c_int, String> {
        unsafe {
            self.library
                .get::<unsafe extern "C" fn(State, *const c_char) -> c_int>(b"lua_getglobal\0")
                .map_err(|e| e.to_string())?(self.state, name.as_ptr());
            let reference = self
                .library
                .get::<unsafe extern "C" fn(State, c_int) -> c_int>(b"luaL_ref\0")
                .map_err(|e| e.to_string())?(self.state, -1001000);
            self.hide_global(name)?;
            Ok(reference)
        }
    }

    fn hide_global(&self, name: &CStr) -> Result<(), String> {
        unsafe {
            self.library
                .get::<unsafe extern "C" fn(State)>(b"lua_pushnil\0")
                .map_err(|e| e.to_string())?(self.state);
            self.library
                .get::<unsafe extern "C" fn(State, *const c_char)>(b"lua_setglobal\0")
                .map_err(|e| e.to_string())?(self.state, name.as_ptr());
            Ok(())
        }
    }

    fn push_function(&self, reference: c_int) -> Result<(), String> {
        unsafe {
            self.library
                .get::<unsafe extern "C" fn(State, c_int, i64) -> c_int>(b"lua_rawgeti\0")
                .map_err(|e| e.to_string())?(self.state, -1001000, i64::from(reference));
        }
        Ok(())
    }

    fn call(&self, arguments: c_int, results: c_int) -> Result<(), String> {
        let status = unsafe {
            self.library.get::<unsafe extern "C" fn(State, c_int, c_int, c_int, isize, *const c_void) -> c_int>(
                b"lua_pcallk\0",
            ).map_err(|e| e.to_string())?(self.state, arguments, results, 0, 0, std::ptr::null())
        };
        if status == 0 {
            Ok(())
        } else {
            Err(self.take_string()?)
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        if self.host.is_some() {
            HOST.with(|slot| slot.set(std::ptr::null_mut()));
        }
        unsafe {
            if let Ok(close) = self
                .library
                .get::<unsafe extern "C" fn(State)>(b"lua_close\0")
            {
                close(self.state)
            }
        }
    }
}
fn literal(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "nil".into(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            let mut out = String::from("\"");
            for b in s.as_bytes() {
                out.push_str(&format!("\\{b:03}"));
            }
            out.push('"');
            out
        }
        serde_json::Value::Array(a) => format!(
            "{{{}}}",
            a.iter().map(literal).collect::<Vec<_>>().join(",")
        ),
        serde_json::Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!(
                    "[{}]={}",
                    literal(&serde_json::Value::String(k.clone())),
                    literal(v)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}
pub fn execute(
    doc: &Document,
    source: &str,
    frame: usize,
    layer: usize,
    command: Option<&str>,
    discover: bool,
    colors: Option<[[u8; 4]; 2]>,
) -> Result<String, String> {
    execute_with_host(
        doc,
        source,
        frame,
        layer,
        command,
        discover,
        colors,
        serde_json::json!({}),
        None,
    )
}
#[expect(
    clippy::too_many_arguments,
    reason = "Preserve the established public API"
)]
pub fn execute_with_host(
    doc: &Document,
    source: &str,
    frame: usize,
    layer: usize,
    command: Option<&str>,
    discover: bool,
    colors: Option<[[u8; 4]; 2]>,
    options: serde_json::Value,
    host: Option<HostCallback>,
) -> Result<String, String> {
    let runtime = ScriptRuntime::new(doc, (frame, layer), colors, options, host, false)?;
    runtime.hide_console_handlers()?;
    runtime.runtime.eval(source)?;
    runtime.finish(command, discover)
}

pub(crate) const MAX_SCRIPT_PIXELS: usize = 1_000_000;

pub(crate) fn execute_headless(
    doc: &Document,
    source: &str,
    context: (usize, usize),
    options: serde_json::Value,
) -> Result<(String, String), String> {
    let runtime = ScriptRuntime::new(doc, context, None, options, None, true)?;
    runtime.hide_console_handlers()?;
    runtime.runtime.eval(source)?;
    let document = runtime.finish(None, false)?;
    Ok((document, runtime.take_output()?))
}

/// One sandbox and one document shared by all chunks in a terminal console.
pub(crate) struct Console {
    script: ScriptRuntime,
    reset_reference: c_int,
    snapshot_reference: c_int,
}

pub(crate) enum ConsoleChunk {
    Incomplete,
    Complete {
        output: String,
        error: Option<String>,
    },
}

impl Console {
    pub(crate) fn new(
        doc: &Document,
        context: (usize, usize),
        options: serde_json::Value,
    ) -> Result<Self, String> {
        let script = ScriptRuntime::new(doc, context, None, options, None, true)?;
        let reset_reference = script.runtime.take_global_function(c"_pw_reset_budget")?;
        let snapshot_reference = script.runtime.take_global_function(c"_pw_snapshot")?;
        Ok(Self {
            script,
            reset_reference,
            snapshot_reference,
        })
    }

    pub(crate) fn eval(&self, source: &str) -> Result<ConsoleChunk, String> {
        self.reset_budget()?;
        let error = match self.script.runtime.load_chunk(source) {
            Ok(()) => self.script.runtime.call(0, 0).err(),
            Err(error) if error.ends_with("near <eof>") => return Ok(ConsoleChunk::Incomplete),
            Err(error) => Some(error),
        };
        self.reset_budget()?;
        let output = self.script.take_output()?;
        Ok(ConsoleChunk::Complete { output, error })
    }

    pub(crate) fn snapshot(&self) -> Result<String, String> {
        self.reset_budget()?;
        self.script.runtime.push_function(self.snapshot_reference)?;
        self.script.runtime.call(0, 1)?;
        self.script.runtime.take_string()
    }

    fn reset_budget(&self) -> Result<(), String> {
        self.script.runtime.push_function(self.reset_reference)?;
        self.script.runtime.call(0, 0)
    }
}

struct ScriptRuntime {
    runtime: Runtime,
    finish_reference: c_int,
    rebind_reference: c_int,
    output_reference: Option<c_int>,
}

impl ScriptRuntime {
    fn new(
        doc: &Document,
        (frame, layer): (usize, usize),
        colors: Option<[[u8; 4]; 2]>,
        options: serde_json::Value,
        host: Option<HostCallback>,
        capture_output: bool,
    ) -> Result<Self, String> {
        doc.validate()?;
        if frame >= doc.durations.len() || layer >= doc.layers.len() {
            return Err("Invalid script context".into());
        }
        if doc.logical_pixels() > MAX_SCRIPT_PIXELS {
            return Err(
                "Native scripts are limited to one million stored pixels per invocation".into(),
            );
        }
        let ui_host = host.is_some();
        let mut host = host;
        let runtime = Runtime::new(Some(Box::new(move |request| {
            if request["op"] == "useTool" {
                crate::lua_tools::use_tool(request)
            } else if request["op"] == "aseProperties" {
                crate::lua_properties::request(request)
            } else if let Some(host) = host.as_mut() {
                host(request)
            } else {
                Err("This operation requires an interactive editor session".into())
            }
        })))?;
        runtime.eval(&format!(
            "_pw_options={}; _pw_has_ui_host={ui_host}",
            literal(&options)
        ))?;
        runtime.eval(&format!(
            "_pw_document={}; _pw_frame={frame}; _pw_layer={layer}; _pw_colors={}",
            literal(&serde_json::to_value(doc).map_err(|e| e.to_string())?),
            literal(&serde_json::json!(
                colors.unwrap_or([[255, 255, 255, 255], [0, 0, 0, 255]])
            ))
        ))?;
        runtime.eval(&format!(
            "{}\n{}\n{}\n{}\n{}",
            include_str!("lua_api.lua"),
            include_str!("lua_tilesets.lua"),
            include_str!("lua_dialog_canvas.lua"),
            include_str!("lua_compat.lua"),
            include_str!("lua_properties.lua")
        ))?;
        let finish_reference = runtime.take_global_function(c"_pw_finish")?;
        let rebind_reference = runtime.take_global_function(c"_pw_rebind_workspace")?;
        let output_reference = if capture_output {
            runtime.eval(include_str!("lua_output.lua"))?;
            Some(runtime.take_global_function(c"_pw_take_output")?)
        } else {
            None
        };
        Ok(Self {
            runtime,
            finish_reference,
            rebind_reference,
            output_reference,
        })
    }

    fn rebind(
        &self,
        doc: &Document,
        (frame, layer): (usize, usize),
        colors: [[u8; 4]; 2],
        options: serde_json::Value,
    ) -> Result<(), String> {
        doc.validate()?;
        if doc.logical_pixels() > MAX_SCRIPT_PIXELS
            || frame >= doc.durations.len()
            || layer >= doc.layers.len()
        {
            return Err("Invalid document or site for the extension session".into());
        }
        let input = serde_json::json!({
            "document":doc, "frame":frame+1, "layer":layer+1,
            "colors":colors, "options":options
        });
        self.runtime.push_function(self.rebind_reference)?;
        self.runtime
            .load_chunk(&format!("return {}", literal(&input)))?;
        self.runtime.call(0, 1)?;
        self.runtime.call(1, 0)
    }

    fn release_workspace(&self) -> Result<(), String> {
        self.runtime.push_function(self.rebind_reference)?;
        self.runtime.call(0, 0)?;
        self.runtime.hide_global(c"_pw_result")
    }

    fn hide_console_handlers(&self) -> Result<(), String> {
        self.runtime.hide_global(c"_pw_reset_budget")?;
        self.runtime.hide_global(c"_pw_snapshot")
    }

    fn finish(&self, command: Option<&str>, discover: bool) -> Result<String, String> {
        self.runtime.push_function(self.finish_reference)?;
        let command = command.unwrap_or("");
        unsafe {
            self.runtime
                .library
                .get::<unsafe extern "C" fn(State, *const c_char, usize) -> *const c_char>(
                    b"lua_pushlstring\0",
                )
                .map_err(|e| e.to_string())?(
                self.runtime.state,
                command.as_ptr().cast(),
                command.len(),
            );
            self.runtime
                .library
                .get::<unsafe extern "C" fn(State, c_int)>(b"lua_pushboolean\0")
                .map_err(|e| e.to_string())?(self.runtime.state, i32::from(discover));
        }
        self.runtime.call(2, 0)?;
        self.runtime.global_string(c"_pw_result")
    }

    fn take_output(&self) -> Result<String, String> {
        let reference = self
            .output_reference
            .ok_or("Lua output capture is unavailable")?;
        self.runtime.push_function(reference)?;
        self.runtime.call(0, 1)?;
        self.runtime.take_string()
    }
}

#[derive(Default)]
pub(crate) struct PluginRuntime {
    state: Option<(String, ScriptRuntime)>,
    host: std::sync::Arc<std::sync::Mutex<Option<HostCallback>>>,
}

impl PluginRuntime {
    pub(crate) fn execute(
        &mut self,
        doc: &Document,
        context: (usize, usize),
        colors: [[u8; 4]; 2],
        options: serde_json::Value,
        (source, command): (&str, &str),
        host: HostCallback,
    ) -> Result<String, String> {
        *self
            .host
            .lock()
            .map_err(|_| "Extension host was interrupted")? = Some(host);
        let result = self.execute_current(doc, context, colors, options, source, command);
        *self
            .host
            .lock()
            .map_err(|_| "Extension host was interrupted")? = None;
        if result.is_err() {
            self.state = None;
        }
        result
    }

    fn execute_current(
        &mut self,
        doc: &Document,
        context: (usize, usize),
        colors: [[u8; 4]; 2],
        options: serde_json::Value,
        source: &str,
        command: &str,
    ) -> Result<String, String> {
        if let Some((_, runtime)) = self.state.as_ref().filter(|(loaded, _)| loaded == source) {
            runtime.rebind(doc, context, colors, options)?;
            let result = runtime.finish(Some(command), false)?;
            runtime.release_workspace()?;
            return Ok(result);
        }
        let slot = self.host.clone();
        let runtime = ScriptRuntime::new(
            doc,
            context,
            Some(colors),
            options,
            Some(Box::new(move |request| {
                slot.lock()
                    .map_err(|_| "Extension host was interrupted")?
                    .as_mut()
                    .ok_or("Extension host is unavailable")?(request)
            })),
            false,
        )?;
        runtime.hide_console_handlers()?;
        runtime.runtime.eval(source)?;
        let result = runtime.finish(Some(command), false)?;
        runtime.release_workspace()?;
        self.state = Some((source.to_owned(), runtime));
        Ok(result)
    }
}
