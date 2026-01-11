import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { assertString } from "../../scripts/updater/assert.ts";
import {
  calculatePlatformHashes,
  fetchText,
  readJsonObjectFile,
  shouldUpdate,
  writeJsonFile,
} from "../../scripts/updater/mod.ts";
import type { JsonValue } from "../../scripts/updater/mod.ts";

const versionUrl = "https://cli.coderabbit.ai/releases/latest/VERSION";

const platforms = {
  "x86_64-linux": "linux-x64",
  "aarch64-linux": "linux-arm64",
  "x86_64-darwin": "darwin-x64",
  "aarch64-darwin": "darwin-arm64",
} as const satisfies Record<string, string>;

async function fetchVersion(): Promise<string> {
  return (await fetchText(versionUrl)).trim();
}

async function main(): Promise<void> {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const hashesFilePath = join(scriptDir, "hashes.json");

  const data = await readJsonObjectFile(hashesFilePath);
  const current = data["version"];
  assertString(current, `${hashesFilePath}: version must be a string`);

  const latest = await fetchVersion();

  console.log(`Current: ${current}, Latest: ${latest}`);

  if (!shouldUpdate(current, latest)) {
    console.log("Already up to date");
    return;
  }

  const urlTemplate = `https://cli.coderabbit.ai/releases/${latest}/coderabbit-{platform}.zip`;
  const hashes = await calculatePlatformHashes(urlTemplate, platforms);

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
