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
        #[cfg(not(target_os = "windows"))]
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
            let call=self.library.get::<unsafe extern "C" fn(State,c_int,c_int,c_int,isize,*const c_void)->c_int>(b"lua_pcallk\0").map_err(|e|e.to_string())?;
            if load(
                self.state,
                source.as_ptr().cast(),
                source.len(),
                c"PixelCraft".as_ptr(),
                c"t".as_ptr(),
            ) != 0
                || call(self.state, 0, 0, 0, 0, std::ptr::null()) != 0
            {
                return Err(self.string_at(-1)?);
            }
            Ok(())
        }
    }
    fn string_at(&self, index: c_int) -> Result<String, String> {
        unsafe {
            let p = self
                .library
                .get::<unsafe extern "C" fn(State, c_int, *mut usize) -> *const c_char>(
                    b"lua_tolstring\0",
                )
                .map_err(|e| e.to_string())?(
                self.state, index, std::ptr::null_mut()
            );
            if p.is_null() {
                return Err("Lua returned no result".into());
            }
            Ok(CStr::from_ptr(p).to_string_lossy().into_owned())
        }
    }
    fn result(&self) -> Result<String, String> {
        unsafe {
            self.library
                .get::<unsafe extern "C" fn(State, *const c_char) -> c_int>(b"lua_getglobal\0")
                .map_err(|e| e.to_string())?(self.state, c"_pw_result".as_ptr());
        }
        self.string_at(-1)
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
    doc.validate()?;
    if frame >= doc.durations.len() || layer >= doc.layers.len() {
        return Err("Invalid script context".into());
    }
    if doc.stored_pixels() > 1_000_000 {
        return Err(
            "Native scripts are limited to one million stored pixels per invocation".into(),
        );
    }
    let runtime = Runtime::new(host)?;
    runtime.eval(&format!("_pw_options={}", literal(&options)))?;
    runtime.eval(&format!(
        "_pw_document={}; _pw_frame={frame}; _pw_layer={layer}; _pw_colors={}",
        literal(&serde_json::to_value(doc).map_err(|e| e.to_string())?),
        literal(&serde_json::json!(
            colors.unwrap_or([[255, 255, 255, 255], [0, 0, 0, 255]])
        ))
    ))?;
    runtime.eval(&format!(
        "{}\n{}",
        include_str!("lua_api.lua"),
        include_str!("lua_compat.lua")
    ))?;
    let finish_ref = unsafe {
        runtime
            .library
            .get::<unsafe extern "C" fn(State, *const c_char) -> c_int>(b"lua_getglobal\0")
            .map_err(|e| e.to_string())?(runtime.state, c"_pw_finish".as_ptr());
        let reference = runtime
            .library
            .get::<unsafe extern "C" fn(State, c_int) -> c_int>(b"luaL_ref\0")
            .map_err(|e| e.to_string())?(runtime.state, -1001000);
        runtime
            .library
            .get::<unsafe extern "C" fn(State)>(b"lua_pushnil\0")
            .map_err(|e| e.to_string())?(runtime.state);
        runtime
            .library
            .get::<unsafe extern "C" fn(State, *const c_char)>(b"lua_setglobal\0")
            .map_err(|e| e.to_string())?(runtime.state, c"_pw_finish".as_ptr());
        reference
    };
    runtime.eval(source)?;
    unsafe {
        runtime
            .library
            .get::<unsafe extern "C" fn(State, c_int, i64) -> c_int>(b"lua_rawgeti\0")
            .map_err(|e| e.to_string())?(runtime.state, -1001000, finish_ref as i64);
        let command = command.unwrap_or("");
        runtime
            .library
            .get::<unsafe extern "C" fn(State, *const c_char, usize) -> *const c_char>(
                b"lua_pushlstring\0",
            )
            .map_err(|e| e.to_string())?(
            runtime.state, command.as_ptr().cast(), command.len()
        );
        runtime
            .library
            .get::<unsafe extern "C" fn(State, c_int)>(b"lua_pushboolean\0")
            .map_err(|e| e.to_string())?(runtime.state, i32::from(discover));
        let status = runtime
            .library
            .get::<unsafe extern "C" fn(State, c_int, c_int, c_int, isize, *const c_void) -> c_int>(
                b"lua_pcallk\0",
            )
            .map_err(|e| e.to_string())?(
            runtime.state, 2, 0, 0, 0, std::ptr::null()
        );
        if status != 0 {
            return Err(runtime.string_at(-1)?);
        }
    }
    runtime.result()
}
