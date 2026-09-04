#!/usr/bin/env node

import { readFileSync, appendFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const SEMVER =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;

function readJson(root, relativePath) {
  return JSON.parse(readFileSync(path.join(root, relativePath), "utf8"));
}

function cargoPackageVersion(source) {
  const section = source.match(
    /^\[package\]\s*((?:(?!^\[)[\s\S])*)/m,
  )?.[1];
  return section?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
}

function cargoLockPackageVersion(source, packageName) {
  const matches = source
    .split(/\n\n+/)
    .filter(
      (block) =>
        block.startsWith("[[package]]") &&
        block.includes(`name = "${packageName}"`) &&
        !/^source = /m.test(block),
    )
    .map((block) => block.match(/^version\s*=\s*"([^"]+)"/m)?.[1])
    .filter((version) => typeof version === "string");
  return matches.length === 1 ? matches[0] : undefined;
}

export function readReleaseVersions(root) {
  const packageJson = readJson(root, "package.json");
  const packageLock = readJson(root, "package-lock.json");
  const tauriConfig = readJson(root, "src-tauri/tauri.conf.json");
  const cargoToml = readFileSync(
    path.join(root, "src-tauri/Cargo.toml"),
    "utf8",
  );
  const cargoLock = readFileSync(
    path.join(root, "src-tauri/Cargo.lock"),
    "utf8",
  );

  return {
    "package.json": packageJson.version,
    "package-lock.json": packageLock.version,
    'package-lock.json packages[""]': packageLock.packages?.[""]?.version,
    "src-tauri/Cargo.toml": cargoPackageVersion(cargoToml),
    'src-tauri/Cargo.lock package "streamline-desktop"':
      cargoLockPackageVersion(cargoLock, "streamline-desktop"),
    "src-tauri/tauri.conf.json": tauriConfig.version,
  };
}

export function validateReleaseVersion(root, tag = "") {
  const versions = readReleaseVersions(root);
  const entries = Object.entries(versions);
  const missing = entries.filter(([, value]) => typeof value !== "string" || value === "");
  if (missing.length > 0) {
    throw new Error(
      `unreadable version metadata: ${missing.map(([name]) => name).join(", ")}`,
    );
  }

  const distinct = [...new Set(entries.map(([, value]) => value))];
  if (distinct.length !== 1) {
    throw new Error(
      `desktop version metadata is not in lockstep: ${entries
        .map(([name, value]) => `${name}=${value}`)
        .join(", ")}`,
    );
  }

  const version = distinct[0];
  const semverMatch = version.match(SEMVER);
  if (
    !semverMatch ||
    semverMatch[4]
      ?.split(".")
      .some((identifier) => /^\d+$/.test(identifier) && /^0\d+/.test(identifier))
  ) {
    throw new Error(`desktop version is not semantic: ${version}`);
  }

  if (tag !== "") {
    if (!tag.startsWith("v")) {
      throw new Error(`release tag must have the form v<version>; got ${tag}`);
    }
    const tagVersion = tag.slice(1);
    if (tagVersion !== version) {
      throw new Error(
        `release tag ${tag} does not match desktop metadata version ${version}`,
      );
    }
  }

  return {
    version,
    prerelease: Boolean(semverMatch[4]),
    versions,
  };
}

function parseArgs(argv) {
  const args = { root: process.cwd(), tag: "", githubOutput: "" };
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    switch (value) {
      case "--root":
        args.root = argv[++index];
        break;
      case "--tag":
        args.tag = argv[++index] ?? "";
        break;
      case "--github-output":
        args.githubOutput = argv[++index];
        break;
      default:
        throw new Error(`unknown argument: ${value}`);
    }
  }
  return args;
}

export function main(argv = process.argv.slice(2)) {
  const args = parseArgs(argv);
  const result = validateReleaseVersion(path.resolve(args.root), args.tag);
  console.log(
    `Desktop release metadata is in lockstep at ${result.version}` +
      (args.tag ? ` and matches ${args.tag}` : ""),
  );
  if (args.githubOutput) {
    appendFileSync(
      args.githubOutput,
      `version=${result.version}\nprerelease=${result.prerelease}\n`,
      "utf8",
    );
  }
  return result;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
