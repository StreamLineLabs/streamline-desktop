import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { validateReleaseVersion } from "../check-release-version.mjs";

const roots = [];

function fixture() {
  const root = mkdtempSync(path.join(os.tmpdir(), "streamline-desktop-version-"));
  roots.push(root);
  cpSync("package.json", path.join(root, "package.json"));
  cpSync("package-lock.json", path.join(root, "package-lock.json"));
  const tauri = path.join(root, "src-tauri");
  mkdirSync(tauri);
  cpSync("src-tauri/Cargo.toml", path.join(tauri, "Cargo.toml"), {
    recursive: false,
    force: true,
  });
  cpSync("src-tauri/Cargo.lock", path.join(tauri, "Cargo.lock"), {
    recursive: false,
    force: true,
  });
  cpSync("src-tauri/tauri.conf.json", path.join(tauri, "tauri.conf.json"), {
    recursive: false,
    force: true,
  });
  return root;
}

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe("desktop release version contract", () => {
  it("accepts the current stable tag when all four authorities and the lock root agree", () => {
    const result = validateReleaseVersion(process.cwd(), "v0.4.0");
    expect(result.version).toBe("0.4.0");
    expect(result.prerelease).toBe(false);
  });

  it("marks a matching semantic prerelease tag as a prerelease", () => {
    const root = fixture();
    for (const file of ["package.json", "src-tauri/tauri.conf.json"]) {
      const pathname = path.join(root, file);
      writeFileSync(
        pathname,
        readFileSync(pathname, "utf8").replaceAll("0.4.0", "0.4.0-rc.1"),
      );
    }
    const lock = path.join(root, "package-lock.json");
    writeFileSync(
      lock,
      readFileSync(lock, "utf8").replaceAll("0.4.0", "0.4.0-rc.1"),
    );
    const cargo = path.join(root, "src-tauri/Cargo.toml");
    writeFileSync(
      cargo,
      readFileSync(cargo, "utf8").replace(
        'version = "0.4.0"',
        'version = "0.4.0-rc.1"',
      ),
    );
    const cargoLock = path.join(root, "src-tauri/Cargo.lock");
    writeFileSync(
      cargoLock,
      readFileSync(cargoLock, "utf8").replace(
        'name = "streamline-desktop"\nversion = "0.4.0"',
        'name = "streamline-desktop"\nversion = "0.4.0-rc.1"',
      ),
    );

    const result = validateReleaseVersion(root, "v0.4.0-rc.1");
    expect(result.version).toBe("0.4.0-rc.1");
    expect(result.prerelease).toBe(true);
  });

  it.each([
    "package.json",
    "package-lock.json",
    'package-lock.json packages[""]',
    "src-tauri/Cargo.toml",
    'src-tauri/Cargo.lock package "streamline-desktop"',
    "src-tauri/tauri.conf.json",
  ])("rejects drift in %s", (authority) => {
    const root = fixture();
    if (authority === "package.json") {
      const file = path.join(root, "package.json");
      writeFileSync(file, readFileSync(file, "utf8").replace("0.4.0", "0.4.1"));
    } else if (authority.startsWith("package-lock")) {
      const file = path.join(root, "package-lock.json");
      const lock = JSON.parse(readFileSync(file, "utf8"));
      if (authority.includes('packages[""]')) {
        lock.packages[""].version = "0.4.1";
      } else {
        lock.version = "0.4.1";
      }
      writeFileSync(file, `${JSON.stringify(lock, null, 2)}\n`);
    } else if (authority.endsWith("Cargo.toml")) {
      const file = path.join(root, "src-tauri/Cargo.toml");
      writeFileSync(file, readFileSync(file, "utf8").replace("0.4.0", "0.4.1"));
    } else if (authority.includes("Cargo.lock")) {
      const file = path.join(root, "src-tauri/Cargo.lock");
      writeFileSync(
        file,
        readFileSync(file, "utf8").replace(
          'name = "streamline-desktop"\nversion = "0.4.0"',
          'name = "streamline-desktop"\nversion = "0.4.1"',
        ),
      );
    } else {
      const file = path.join(root, "src-tauri/tauri.conf.json");
      writeFileSync(file, readFileSync(file, "utf8").replace("0.4.0", "0.4.1"));
    }

    expect(() => validateReleaseVersion(root, "v0.4.0")).toThrow(
      /not in lockstep/,
    );
  });
});
