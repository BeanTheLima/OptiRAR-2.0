#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use optirar_gui::{LaunchArgs, QuickAction};
use std::path::PathBuf;

/// optirar-desktop [--extract-dialog | --create | --extract-here | --extract-folder | --test | --add-zip] [FILES...]
fn parse_args(args: impl Iterator<Item = std::ffi::OsString>) -> LaunchArgs {
    let mut launch = LaunchArgs::default();
    for arg in args {
        match arg.to_str() {
            Some("--extract-dialog") => launch.open_extract_dialog = true,
            Some("--create") => launch.open_create_dialog = true,
            Some("--extract-here") => launch.quick = Some(QuickAction::ExtractHere),
            Some("--extract-folder") => launch.quick = Some(QuickAction::ExtractToFolder),
            Some("--test") => launch.quick = Some(QuickAction::Test),
            Some("--add-zip") => launch.quick = Some(QuickAction::AddZip),
            Some(s) if s.starts_with("--") => {}
            // OsString keeps any Windows path (Vietnamese names included) intact.
            _ => launch.files.push(PathBuf::from(arg)),
        }
    }
    launch
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let mut launch = parse_args(std::env::args_os().skip(1));
    // Explorer starts one process per selected file: merge them into one window/job.
    let batch_key = match launch.quick {
        Some(q) => Some(format!("{q:?}")),
        None if launch.open_create_dialog => Some("create".to_string()),
        None => None,
    };
    if let Some(key) = batch_key {
        match optirar_gui::batch::collect(&key, std::mem::take(&mut launch.files)) {
            Some(all) => launch.files = all,
            None => return Ok(()), // another process handles our files
        }
    }
    optirar_gui::run_gui(launch)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_flags_and_files_in_any_order() {
        let a = super::parse_args(["C:\\tệp nén.zip", "--extract-here"].iter().map(Into::into));
        assert_eq!(a.quick, Some(optirar_gui::QuickAction::ExtractHere));
        assert_eq!(a.files.len(), 1);
    }
}
