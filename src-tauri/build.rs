use std::path::Path;
use std::process::Command;

fn main() {
    build_ocr_sidecar();
    tauri_build::build();
}

// Compile the Apple Vision OCR sidecar from Swift, if the toolchain is present.
// Non-fatal: without swiftc (Xcode Command Line Tools), Zortbit still runs —
// OCR is simply disabled and files fall back to name/type-based filing.
fn build_ocr_sidecar() {
    let manifest = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(m) => m,
        Err(_) => return,
    };
    println!("cargo:rerun-if-changed=ocr/zb_ocr.swift");
    let swift = Path::new(&manifest).join("ocr/zb_ocr.swift");
    let bin = Path::new(&manifest).join("bin/zb_ocr");
    if !swift.exists() {
        return;
    }
    if bin.exists() && !is_newer(&swift, &bin) {
        return;
    }
    let _ = std::fs::create_dir_all(Path::new(&manifest).join("bin"));
    let status = Command::new("swiftc").arg("-O").arg(&swift).arg("-o").arg(&bin).status();
    match status {
        Ok(s) if s.success() => {}
        _ => println!(
            "cargo:warning=zb_ocr OCR sidecar not built (swiftc / Xcode CLT unavailable) — OCR disabled; name + type filing still work"
        ),
    }
}

fn is_newer(a: &Path, b: &Path) -> bool {
    match (
        a.metadata().and_then(|m| m.modified()),
        b.metadata().and_then(|m| m.modified()),
    ) {
        (Ok(ta), Ok(tb)) => ta > tb,
        _ => true,
    }
}
