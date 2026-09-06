import {
  chmodSync,
  copyFileSync,
  mkdirSync,
} from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";
import path from "node:path";

const coreDir = process.env.STREAMLINE_CORE_DIR ?? path.resolve("..", "streamline");
const target =
  process.env.TARGET ??
  execFileSync("rustc", ["-vV"], { encoding: "utf8" })
    .match(/^host: (.+)$/m)?.[1];

if (!target) {
  console.error("Could not determine the Rust target triple.");
  process.exit(1);
}

const cargo = spawnSync(
  "cargo",
  [
    "build",
    "--manifest-path",
    path.join(coreDir, "Cargo.toml"),
    "--locked",
    "--release",
    "--target",
    target,
    "--features",
    "schema-registry",
    "--bin",
    "streamline",
  ],
  { stdio: "inherit" },
);

if (cargo.status !== 0) {
  process.exit(cargo.status ?? 1);
}

const extension = target.includes("windows") ? ".exe" : "";
const source = path.join(
  coreDir,
  "target",
  target,
  "release",
  `streamline${extension}`,
);
const destinationDir = path.join("src-tauri", "binaries");
const destination = path.join(
  destinationDir,
  `streamline-${target}${extension}`,
);

mkdirSync(destinationDir, { recursive: true });
copyFileSync(source, destination);
if (!extension) {
  chmodSync(destination, 0o755);
}
console.log(`Prepared ${destination}`);
