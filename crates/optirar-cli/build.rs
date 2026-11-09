//! Embeds assets/optirar.rc (icon + version info) into the executable (GNU/MinGW toolchain).
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let rc = assets.join("optirar.rc");
    println!("cargo:rerun-if-changed={}", rc.display());
    println!("cargo:rerun-if-changed={}", assets.join("optirar.ico").display());

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu") {
        println!("cargo:warning=icon not embedded: only the GNU toolchain (windres) is wired up");
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("optirar_res.o");
    let candidates = [std::env::var("WINDRES").unwrap_or_default(), "x86_64-w64-mingw32-windres".into(), "windres".into()];
    for windres in candidates.iter().filter(|w| !w.is_empty()) {
        // Relative input + cwd: windres hands the path to a preprocessor that breaks on spaces.
        let ok = Command::new(windres)
            .current_dir(&assets)
            .arg("--input")
            .arg("optirar.rc")
            .arg("--output-format=coff")
            .arg("--output")
            .arg(&out)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            println!("cargo:rustc-link-arg-bins={}", out.display());
            return;
        }
    }
    println!("cargo:warning=icon not embedded: windres not found (add llvm-mingw\\bin to PATH)");
}
