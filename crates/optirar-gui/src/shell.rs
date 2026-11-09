//! Small OS helpers: temp folders, opening files, drag-out and Explorer registration.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Per-process temp folder; removed on exit.
pub fn session_dir() -> PathBuf {
    std::env::temp_dir().join("OptiRAR").join(std::process::id().to_string())
}

/// Fresh, unique sub-folder of the session dir (e.g. for one drag or one "open").
pub fn new_temp_dir(kind: &str) -> PathBuf {
    session_dir().join(format!("{}-{}", kind, COUNTER.fetch_add(1, Ordering::Relaxed)))
}

pub fn cleanup_session() {
    let _ = std::fs::remove_dir_all(session_dir());
}

/// Open a file with its associated application, or a folder in Explorer.
pub fn open_with_system(path: &Path) {
    #[cfg(windows)]
    let _ = Command::new("explorer.exe").arg(path).spawn();
    #[cfg(not(windows))]
    let _ = Command::new("xdg-open").arg(path).spawn();
}

/// Show `path` selected in an Explorer window.
pub fn reveal_in_explorer(path: &Path) {
    #[cfg(windows)]
    {
        if path.is_dir() {
            let _ = Command::new("explorer.exe").arg(path).spawn();
        } else {
            let _ = Command::new("explorer.exe").raw_arg(format!("/select,\"{}\"", path.display())).spawn();
        }
    }
    #[cfg(not(windows))]
    open_with_system(path);
}

/// Start an OS drag-and-drop of real files out of our window (blocks until drop/cancel).
#[cfg(windows)]
pub fn start_drag(frame: &eframe::Frame, files: Vec<PathBuf>) -> Result<(), String> {
    drag::start_drag(frame, drag::DragItem::Files(files), drag::Image::Raw(Vec::new()), |_, _| {}, drag::Options::default())
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn start_drag(_frame: &eframe::Frame, _files: Vec<PathBuf>) -> Result<(), String> {
    Err("Drag-out is only supported on Windows".into())
}

/// Run the bundled Explorer (un)registration script for this executable.
/// `set_default` also makes OptiRAR the double-click program for archives (WinRAR behaviour).
pub fn run_shell_script(register: bool, lang: &str, set_default: bool) -> Result<(), String> {
    let (name, script) = if register {
        ("optirar_register_shell.ps1", include_str!("../../../scripts/register_shell.ps1"))
    } else {
        ("optirar_unregister_shell.ps1", include_str!("../../../scripts/unregister_shell.ps1"))
    };
    let temp_script = std::env::temp_dir().join(name);
    // UTF-8 BOM so Windows PowerShell 5.1 reads non-ASCII menu labels correctly.
    std::fs::write(&temp_script, [b"\xEF\xBB\xBF".as_slice(), script.trim_start_matches('\u{feff}').as_bytes()].concat()).map_err(|e| e.to_string())?;

    let gui = std::env::current_exe().map_err(|e| e.to_string())?;

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]).arg(&temp_script);
    if register {
        cmd.arg("-OptiRarGui").arg(&gui).arg("-Lang").arg(lang);
        if set_default {
            cmd.arg("-SetDefault");
        }
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd.output().map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&temp_script);
    let out = out?;
    if out.status.success() {
        Ok(())
    } else {
        let mut msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if msg.is_empty() {
            msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
        Err(msg)
    }
}
