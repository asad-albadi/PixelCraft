use super::{require_script_file, script_absolute_path};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Stdio},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Launch<'a> {
    executable: &'a str,
    #[serde(default, borrow)]
    args: Vec<&'a str>,
    working_directory: Option<&'a str>,
}

/// Start one explicitly approved executable with a literal argument list.
pub fn script_process_request(
    base: &Path,
    request: &Value,
    authorize: impl FnOnce(&str) -> Result<(), String>,
) -> Result<Value, String> {
    let launch = Launch::deserialize(request).map_err(|error| error.to_string())?;
    let executable = Path::new(launch.executable);
    if !executable.is_absolute() {
        return Err("Lua process launch requires an absolute executable path".into());
    }
    let executable = script_absolute_path(executable)?;
    #[cfg(windows)]
    if !executable
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return Err("Lua process launch on Windows requires an .exe file".into());
    }
    let workdir = script_absolute_path(&base.join(launch.working_directory.unwrap_or("")))?;
    if launch.args.len() > 128
        || launch.args.iter().map(|arg| arg.len()).sum::<usize>() > 16_384
        || launch.args.iter().any(|arg| arg.contains('\0'))
    {
        return Err("Invalid Lua process arguments".into());
    }
    authorize(&format!(
        "Program: {executable:?}\nWorking directory: {workdir:?}\nArguments: {:?}",
        launch.args
    ))?;
    require_script_file(&executable, false)?;
    let mut command = Command::new(executable);
    command
        .current_dir(workdir)
        .args(launch.args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
        if let Some(system_root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", system_root);
        }
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not launch Lua helper: {error}"))?;
    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(json!({"pid":pid}))
}
