fn main() {
    // Bump this number whenever godly-mcp code changes.
    // Shows in the log as "build=N" so we can tell which binary is running.
    println!("cargo:rustc-env=GODLY_MCP_BUILD=4");
}
