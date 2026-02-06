fn main() {
    // Build the daemon binary alongside the Tauri app.
    // This ensures godly-daemon.exe is in the target directory when the app launches.
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let status = std::process::Command::new("cargo")
        .args(["build", "-p", "godly-daemon"])
        .args(if profile == "release" {
            vec!["--release"]
        } else {
            vec![]
        })
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("cargo:warning=godly-daemon built successfully ({profile})");
        }
        Ok(s) => {
            println!("cargo:warning=godly-daemon build failed with exit code: {s}");
        }
        Err(e) => {
            println!("cargo:warning=Failed to build godly-daemon: {e}");
        }
    }

    tauri_build::build()
}
