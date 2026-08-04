import { readdir } from "node:fs/promises";

const directory = "src-tauri/binaries";

try {
  const files = await readdir(directory);
  if (!files.some((file) => /^streamline-.+(?:\.exe)?$/.test(file))) {
    throw new Error("no target-specific sidecar");
  }
} catch {
  console.error(`Missing target-specific Streamline sidecar under ${directory}`);
  console.error("Build the matching core release binary before packaging the desktop app.");
  process.exit(1);
}
