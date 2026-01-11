import { Buffer } from "node:buffer";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { assertArray, assertRecord, assertString } from "../../scripts/updater/assert.ts";
import {
  fetchJson,
  readJsonObjectFile,
  shouldUpdate,
  writeJsonFile,
} from "../../scripts/updater/mod.ts";
import type { JsonValue } from "../../scripts/updater/mod.ts";

const manifestUrl =
  "https://qoder-ide.oss-ap-southeast-1.aliyuncs.com/qodercli/channels/manifest.json";

const platformMap = {
  "linux/amd64": "x86_64-linux",
  "linux/arm64": "aarch64-linux",
  "darwin/amd64": "x86_64-darwin",
  "darwin/arm64": "aarch64-darwin",
} as const satisfies Record<string, string>;

function hexToSriSha256(hex: string): string {
  if (!/^[0-9a-fA-F]{64}$/.test(hex)) {
    throw new Error(`Invalid sha256 hex: ${JSON.stringify(hex)}`);
  }
  const bytes = Buffer.from(hex, "hex");
  return `sha256-${bytes.toString("base64")}`;
}

type ManifestFileEntry = Readonly<{
  os: string;
  arch: string;
  sha256: string;
}>;

function parseManifestFileEntry(value: unknown, context: string): ManifestFileEntry {
  assertRecord(value, `${context}: expected object`);
  const os = value["os"];
  const arch = value["arch"];
  const sha256 = value["sha256"];
  assertString(os, `${context}: expected os string`);
  assertString(arch, `${context}: expected arch string`);
  assertString(sha256, `${context}: expected sha256 string`);
  return { os, arch, sha256 };
}

async function main(): Promise<void> {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const hashesFilePath = join(scriptDir, "hashes.json");

  const data = await readJsonObjectFile(hashesFilePath);
  const current = data["version"];
  assertString(current, `${hashesFilePath}: version must be a string`);

  console.log("Fetching manifest from official source...");
  const response = await fetchJson(manifestUrl);
  assertRecord(response, "manifest: expected JSON object");

  const latest = response["latest"];
  assertString(latest, "manifest.latest must be a string");

  console.log(`Current: ${current}, Latest: ${latest}`);

  if (!shouldUpdate(current, latest)) {
    console.log("Already up to date");
    return;
  }

  const files = response["files"];
  assertArray(files, "manifest.files must be an array");

  const hashes: Record<string, string> = {};

  for (let i = 0; i < files.length; i += 1) {
    const entry = parseManifestFileEntry(files[i], `manifest.files[${i}]`);
    const key = `${entry.os}/${entry.arch}`;
    const nixPlatform = platformMap[key as keyof typeof platformMap];
    if (!nixPlatform) continue;
    hashes[nixPlatform] = hexToSriSha256(entry.sha256);
  }

  const expected = new Set(Object.values(platformMap));
  const got = new Set(Object.keys(hashes));
  const missing = [...expected].filter((p) => !got.has(p));
  if (missing.length > 0) {
    console.log(`Warning: Missing platforms in manifest: ${missing.join(", ")}`);
  }

  const nextData: Record<string, JsonValue> = {
    version: latest,
    hashes,
  };
  await writeJsonFile(hashesFilePath, nextData);

  console.log(`Updated to ${latest}`);
}

if (import.meta.main) {
  await main();
}
