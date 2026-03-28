fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../images/engram.ico");
        res.set("ProductName", "ENGRAM");
        res.set("FileDescription", "ENGRAM — Engineering Intelligence Platform");
        res.set("LegalCopyright", "Copyright (c) 2026 Manoj Pisini");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to set Windows icon: {e}");
        }
    }
}
