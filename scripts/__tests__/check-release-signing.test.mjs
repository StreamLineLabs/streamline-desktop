import { describe, it, expect } from "vitest";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import {
  REQUIRED_SIGNING_VARS,
  checkReleaseSigning,
  formatMissingVarsError,
  isUnsignedBuildAllowed,
  missingSigningVars,
  requiredSigningVars,
} from "../check-release-signing.mjs";

// Vitest transforms this module, so `import.meta.url` is not a file URL here;
// resolve from the project root that Vitest uses as the working directory.
const SCRIPT_PATH = path.resolve(process.cwd(), "scripts/check-release-signing.mjs");

/** Every signing variable the script knows about, across all platforms. */
const ALL_SIGNING_VARS = Object.values(REQUIRED_SIGNING_VARS).flat();

/**
 * Run the script as a real CLI with a clean signing environment, so the test
 * asserts the process exit code rather than the exported helpers.
 */
function runCli(args, overrides = {}) {
  const env = { ...process.env };
  for (const name of [...ALL_SIGNING_VARS, "ALLOW_UNSIGNED_BUILD"]) {
    delete env[name];
  }
  return spawnSync(process.execPath, [SCRIPT_PATH, ...args], {
    encoding: "utf8",
    env: { ...env, ...overrides },
  });
}

const APPLE_ENV = {
  APPLE_CERTIFICATE: "base64",
  APPLE_CERTIFICATE_PASSWORD: "pw",
  APPLE_SIGNING_IDENTITY: "Developer ID Application: Example",
  APPLE_ID: "release@example.com",
  APPLE_PASSWORD: "app-specific",
  APPLE_TEAM_ID: "TEAMID",
};

describe("release signing preflight", () => {
  it("requires the documented Apple variables on macOS", () => {
    expect(requiredSigningVars("darwin")).toContain("APPLE_SIGNING_IDENTITY");
    expect(requiredSigningVars("darwin")).toContain("APPLE_TEAM_ID");
  });

  it("requires the documented Windows certificate variables", () => {
    expect(requiredSigningVars("win32")).toEqual([
      "WINDOWS_CERTIFICATE",
      "WINDOWS_CERTIFICATE_PASSWORD",
      "WINDOWS_CERTIFICATE_THUMBPRINT",
    ]);
  });

  it("requires nothing on Linux, which Tauri does not code-sign", () => {
    expect(requiredSigningVars("linux")).toEqual([]);
    const result = checkReleaseSigning("linux", {});
    expect(result.ok).toBe(true);
    expect(result.signed).toBe(false);
  });

  it("treats unset and blank variables as missing", () => {
    const missing = missingSigningVars("darwin", {
      ...APPLE_ENV,
      APPLE_TEAM_ID: "   ",
      APPLE_ID: undefined,
    });
    expect(missing).toEqual(["APPLE_ID", "APPLE_TEAM_ID"]);
  });

  it("fails closed when signing variables are absent", () => {
    const result = checkReleaseSigning("darwin", {});
    expect(result.ok).toBe(false);
    expect(result.signed).toBe(false);
    expect(result.message).toContain("APPLE_CERTIFICATE");
    expect(result.message).toContain("ALLOW_UNSIGNED_BUILD");
  });

  it("passes when every signing variable is present", () => {
    const result = checkReleaseSigning("darwin", APPLE_ENV);
    expect(result.ok).toBe(true);
    expect(result.signed).toBe(true);
  });

  it("allows an explicitly opted-in unsigned development build", () => {
    const result = checkReleaseSigning("win32", { ALLOW_UNSIGNED_BUILD: "true" });
    expect(result.ok).toBe(true);
    expect(result.signed).toBe(false);
    expect(result.message).toContain("UNSIGNED development build");
  });

  it("only honours an exact opt-in value", () => {
    expect(isUnsignedBuildAllowed({ ALLOW_UNSIGNED_BUILD: "TRUE" })).toBe(true);
    expect(isUnsignedBuildAllowed({ ALLOW_UNSIGNED_BUILD: "1" })).toBe(false);
    expect(isUnsignedBuildAllowed({})).toBe(false);
    expect(checkReleaseSigning("win32", { ALLOW_UNSIGNED_BUILD: "1" }).ok).toBe(false);
  });

  it("never echoes secret values in the failure message", () => {
    const message = formatMissingVarsError("darwin", ["APPLE_CERTIFICATE"]);
    expect(message).not.toContain("base64");
    expect(message).toContain("Missing required signing variables");
  });
});

describe("release signing preflight CLI", () => {
  const WINDOWS_ENV = {
    WINDOWS_CERTIFICATE: "base64",
    WINDOWS_CERTIFICATE_PASSWORD: "pw",
    WINDOWS_CERTIFICATE_THUMBPRINT: "ABCDEF0123456789",
  };

  it("exits nonzero when win32 signing variables are missing", () => {
    const result = runCli(["--platform", "win32"]);
    expect(result.status).not.toBe(0);
    expect(result.status).toBe(1);
    expect(result.stderr).toContain("Release signing preflight failed for win32");
    expect(result.stderr).toContain("WINDOWS_CERTIFICATE_THUMBPRINT");
    expect(result.stdout).toBe("");
  });

  it("exits zero when every win32 signing variable is present", () => {
    const result = runCli(["--platform", "win32"], WINDOWS_ENV);
    expect(result.status).toBe(0);
    expect(result.stdout).toContain("Release signing preflight passed for win32");
    expect(result.stderr).toBe("");
  });

  it("exits zero and labels the build when unsigned mode is opted into", () => {
    const result = runCli(["--platform", "win32"], { ALLOW_UNSIGNED_BUILD: "true" });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain("UNSIGNED development build for win32");
    expect(result.stdout).toContain("must not be published as a release");
  });

  it("exits nonzero for a near-miss opt-in value", () => {
    const result = runCli(["--platform", "win32"], { ALLOW_UNSIGNED_BUILD: "1" });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain("Release signing preflight failed for win32");
  });

  it("exits zero on linux, which requires no signing variables", () => {
    const result = runCli(["--platform", "linux"]);
    expect(result.status).toBe(0);
    expect(result.stdout).toContain("No code-signing variables are required for linux");
  });

  it("never prints a secret value, even when signing succeeds", () => {
    const failure = runCli(["--platform", "darwin"]);
    const success = runCli(["--platform", "win32"], WINDOWS_ENV);
    for (const output of [failure.stdout, failure.stderr, success.stdout, success.stderr]) {
      expect(output).not.toContain("ABCDEF0123456789");
      expect(output).not.toContain("base64");
    }
  });

  // The CLI only runs when executed directly. Comparing `import.meta.url` to a
  // hand-built `file://${process.argv[1]}` never matches on Windows, where
  // argv[1] is `C:\...`, so the preflight would silently no-op there.
  it("detects direct execution with pathToFileURL rather than string concatenation", () => {
    const source = readFileSync(SCRIPT_PATH, "utf8");
    expect(source).toContain("pathToFileURL(process.argv[1]).href");
    expect(source).not.toContain("`file://${process.argv[1]}`");

    const windowsArgv = "C:\\a b\\scripts\\check-release-signing.mjs";
    expect(pathToFileURL(windowsArgv).href).not.toBe(`file://${windowsArgv}`);
    expect(pathToFileURL(SCRIPT_PATH).href.startsWith("file://")).toBe(true);
  });
});
