#!/usr/bin/env node

import { readFileSync, writeFileSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");

const SEMVER_RE = /^\d+\.\d+\.\d+$/;

const VERSION_FILES = [
  {
    path: "package.json",
    type: "json",
    field: "version",
  },
  {
    path: "src-tauri/tauri.conf.json",
    type: "json",
    field: "version",
  },
  {
    path: "src-tauri/Cargo.toml",
    type: "cargo",
  },
  {
    path: "src-tauri/protocol/Cargo.toml",
    type: "cargo",
  },
  {
    path: "src-tauri/daemon/Cargo.toml",
    type: "cargo",
  },
  {
    path: "src-tauri/mcp/Cargo.toml",
    type: "cargo",
  },
  {
    path: "src-tauri/godly-vt/Cargo.toml",
    type: "cargo",
  },
  {
    path: "src-tauri/notify/Cargo.toml",
    type: "cargo",
  },
];

function readCurrentVersion() {
  const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf-8"));
  return pkg.version;
}

function bumpVersion(current, bump) {
  if (SEMVER_RE.test(bump)) {
    return bump;
  }

  const [major, minor, patch] = current.split(".").map(Number);

  switch (bump) {
    case "patch":
      return `${major}.${minor}.${patch + 1}`;
    case "minor":
      return `${major}.${minor + 1}.0`;
    case "major":
      return `${major + 1}.0.0`;
    default:
      console.error(
        `Error: Invalid bump type "${bump}". Use patch, minor, major, or an explicit X.Y.Z version.`
      );
      process.exit(1);
  }
}

function updateJsonFile(filePath, field, newVersion) {
  const fullPath = join(root, filePath);
  const content = readFileSync(fullPath, "utf-8");
  const json = JSON.parse(content);
  json[field] = newVersion;
  writeFileSync(fullPath, JSON.stringify(json, null, 2) + "\n", "utf-8");
}

function readCargoVersion(filePath) {
  const fullPath = join(root, filePath);
  const content = readFileSync(fullPath, "utf-8");
  const packageRe = /\[package\][\s\S]*?version\s*=\s*"([^"]+)"/;
  const match = content.match(packageRe);
  if (!match) {
    console.error(`Error: Could not find version in [package] section of ${filePath}`);
    process.exit(1);
  }
  return match[1];
}

function updateCargoToml(filePath, newVersion) {
  const fullPath = join(root, filePath);
  let content = readFileSync(fullPath, "utf-8");

  // Replace the first `version = "X.Y.Z"` under [package]
  const packageRe = /(\[package\][\s\S]*?version\s*=\s*")([^"]+)(")/;
  const match = content.match(packageRe);

  if (!match) {
    console.error(`Error: Could not find version in [package] section of ${filePath}`);
    process.exit(1);
  }

  content = content.replace(packageRe, `$1${newVersion}$3`);
  writeFileSync(fullPath, content, "utf-8");
}

// --- Main ---

const bump = process.argv[2];

if (!bump) {
  console.error("Usage: npm run version:bump -- <patch|minor|major|X.Y.Z>");
  process.exit(1);
}

const oldVersion = readCurrentVersion();
const newVersion = bumpVersion(oldVersion, bump);

if (oldVersion === newVersion) {
  console.log(`Version is already ${oldVersion}, nothing to do.`);
  process.exit(0);
}

console.log(`Bumping version: ${oldVersion} -> ${newVersion}\n`);

for (const file of VERSION_FILES) {
  if (file.type === "json") {
    updateJsonFile(file.path, file.field, newVersion);
    console.log(`  Updated ${file.path} -> ${newVersion}`);
  } else {
    const crateOld = readCargoVersion(file.path);
    const crateNew = bumpVersion(crateOld, bump);
    updateCargoToml(file.path, crateNew);
    console.log(`  Updated ${file.path}: ${crateOld} -> ${crateNew}`);
  }
}

console.log(`\nAll ${VERSION_FILES.length} files updated to ${newVersion}.`);
