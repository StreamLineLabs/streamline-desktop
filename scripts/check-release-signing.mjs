// Release signing preflight.
//
// Packaged releases must be signed (and, on macOS, notarized). This check runs
// before the bundler so a release fails fast with an actionable message instead
// of publishing silently unsigned installers.
//
// Only documented Tauri / platform variable names are used here; this script
// never reads or prints secret values, it only checks that they are present.

import { pathToFileURL } from "node:url";

/** Documented signing variables required per platform. */
export const REQUIRED_SIGNING_VARS = {
  darwin: [
    "APPLE_CERTIFICATE",
    "APPLE_CERTIFICATE_PASSWORD",
    "APPLE_SIGNING_IDENTITY",
    "APPLE_ID",
    "APPLE_PASSWORD",
    "APPLE_TEAM_ID",
  ],
  win32: [
    "WINDOWS_CERTIFICATE",
    "WINDOWS_CERTIFICATE_PASSWORD",
    "WINDOWS_CERTIFICATE_THUMBPRINT",
  ],
  // Tauri does not code-sign Linux bundles; nothing to require.
  linux: [],
};

export function requiredSigningVars(platform) {
  return REQUIRED_SIGNING_VARS[platform] ?? [];
}

/** Names of required variables that are unset or blank. Values are never returned. */
export function missingSigningVars(platform, env = {}) {
  return requiredSigningVars(platform).filter(
    (name) => typeof env[name] !== "string" || env[name].trim() === "",
  );
}

export function isUnsignedBuildAllowed(env = {}) {
  return String(env.ALLOW_UNSIGNED_BUILD ?? "").toLowerCase() === "true";
}

export function formatMissingVarsError(platform, missing) {
  return [
    `Release signing preflight failed for ${platform}.`,
    `Missing required signing variables: ${missing.join(", ")}.`,
    "Configure them as repository/organization secrets exposed to this job, or set",
    "ALLOW_UNSIGNED_BUILD=true to produce clearly-labelled unsigned development artifacts",
    "(never for a published release).",
  ].join("\n");
}

/**
 * @returns {{ ok: boolean, signed: boolean, message: string }}
 */
export function checkReleaseSigning(platform, env = {}) {
  const missing = missingSigningVars(platform, env);

  if (missing.length === 0) {
    const required = requiredSigningVars(platform);
    return {
      ok: true,
      signed: required.length > 0,
      message:
        required.length > 0
          ? `Release signing preflight passed for ${platform}.`
          : `No code-signing variables are required for ${platform}.`,
    };
  }

  if (isUnsignedBuildAllowed(env)) {
    return {
      ok: true,
      signed: false,
      message:
        `UNSIGNED development build for ${platform}: ${missing.join(", ")} not set. ` +
        "Artifacts from this run must not be published as a release.",
    };
  }

  return { ok: false, signed: false, message: formatMissingVarsError(platform, missing) };
}

function main() {
  const platformArgIndex = process.argv.indexOf("--platform");
  const platform =
    platformArgIndex !== -1 ? process.argv[platformArgIndex + 1] : process.platform;

  const result = checkReleaseSigning(platform, process.env);
  if (!result.ok) {
    console.error(result.message);
    process.exit(1);
  }
  console.log(result.message);
}

// Only run the CLI when executed directly, so the checks stay unit-testable.
// `pathToFileURL` is required for correctness on Windows, where `process.argv[1]`
// is a drive path (`C:\...`) that never equals a naive `file://` concatenation.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
