fn main() {
    // Read the app version from package.json (single source of truth)
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // native/
        .and_then(|p| p.parent()) // src-tauri/
        .and_then(|p| p.parent()) // repo root
        .expect("cannot find repo root");

    let package_json = std::fs::read_to_string(repo_root.join("package.json"))
        .expect("cannot read package.json");
    let parsed: serde_json::Value =
        serde_json::from_str(&package_json).expect("cannot parse package.json");
    let version = parsed["version"]
        .as_str()
        .expect("package.json missing version field");

    println!("cargo:rustc-env=GODLY_APP_VERSION={version}");
    println!("cargo:rerun-if-changed=../../../package.json");
}
