fn main() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let def_path = std::path::Path::new(&dir).join("asteria_msi_action.def");
        println!("cargo:rerun-if-changed={}", def_path.display());
        println!("cargo:rustc-cdylib-link-arg=/DEF:{}", def_path.display());
    }
}
