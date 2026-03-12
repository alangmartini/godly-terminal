fn main() {
    // Read the app version from version.txt (single source of truth)
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // native/
        .and_then(|p| p.parent()) // src-tauri/
        .and_then(|p| p.parent()) // repo root
        .expect("cannot find repo root");

    let version = std::fs::read_to_string(repo_root.join("version.txt"))
        .expect("cannot read version.txt")
        .trim()
        .to_string();

    println!("cargo:rustc-env=GODLY_APP_VERSION={version}");
    println!("cargo:rerun-if-changed=../../../version.txt");
}
