use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = Path::new(&manifest_dir);

    // Copy dashboard and config template into crate directory for cargo package
    let workspace_dashboard = manifest_path.join("../../dashboard");
    let local_dashboard = manifest_path.join("dashboard");
    if workspace_dashboard.exists() && !local_dashboard.exists() {
        copy_dir(&workspace_dashboard, &local_dashboard);
    }

    let workspace_config = manifest_path.join("../../engram.toml.example");
    let local_config = manifest_path.join("engram.toml.example");
    if workspace_config.exists() && !local_config.exists() {
        std::fs::copy(&workspace_config, &local_config).ok();
    }

    // Windows executable icon
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let ico = manifest_path.join("../../images/engram.ico");
        if ico.exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(ico.to_str().unwrap());
            res.set("ProductName", "ENGRAM");
            res.set("FileDescription", "ENGRAM — Engineering Intelligence Platform");
            res.set("LegalCopyright", "Copyright (c) 2026 Manoj Pisini");
            if let Err(e) = res.compile() {
                eprintln!("cargo:warning=Failed to set Windows icon: {e}");
            }
        }
    }
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).ok();
    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let path = entry.path();
            let dest = dst.join(entry.file_name());
            if path.is_dir() {
                copy_dir(&path, &dest);
            } else {
                // Skip demo.js — it should not be in the package
                if entry.file_name() == "demo.js" {
                    continue;
                }
                std::fs::copy(&path, &dest).ok();
            }
        }
    }
}
