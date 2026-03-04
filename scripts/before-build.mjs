// Runs cargo sidecar build and vite frontend build in parallel.
// Used as Tauri's beforeBuildCommand for faster production builds.
import { execSync, spawn } from "child_process";

const sidecars = [
  "godly-daemon", "godly-mcp", "godly-notify",
  "godly-pty-shim", "godly-remote", "godly-whisper",
  "godly-iced-shell",
];

function run(cmd, opts = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, { shell: true, stdio: "inherit", ...opts });
    child.on("close", (code) =>
      code === 0 ? resolve() : reject(new Error(`"${cmd}" exited with ${code}`))
    );
  });
}

const cargo = run(
  `cd src-tauri && cargo build ${sidecars.map((s) => `-p ${s}`).join(" ")} --release`
);
const vite = run("npx vite build");

const results = await Promise.allSettled([cargo, vite]);
const failed = results.filter((r) => r.status === "rejected");
if (failed.length) {
  for (const f of failed) console.error(f.reason.message);
  process.exit(1);
}
