import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { assertString } from "../../scripts/updater/assert.ts";
import {
  calculatePlatformHashes,
  fetchVersionFromText,
  readJsonObjectFile,
  shouldUpdate,
  writeJsonFile,
} from "../../scripts/updater/mod.ts";
import type { JsonValue } from "../../scripts/updater/mod.ts";

const platforms = {
  "x86_64-linux": "linux/x64",
  "aarch64-linux": "linux/arm64",
  "aarch64-darwin": "darwin/arm64",
} as const satisfies Record<string, string>;

const versionUrl = "https://app.factory.ai/cli";
const versionPattern = 'VER="([^"]+)"';

async function main(): Promise<void> {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const hashesFilePath = join(scriptDir, "hashes.json");

  const data = await readJsonObjectFile(hashesFilePath);
  const current = data["version"];
  assertString(current, `${hashesFilePath}: version must be a string`);

  const latest = await fetchVersionFromText(versionUrl, versionPattern);

  console.log(`Current: ${current}, Latest: ${latest}`);

  if (!shouldUpdate(current, latest)) {
    console.log("Already up to date");
    return;
  }

  const droidUrlTemplate =
    "https://downloads.factory.ai/factory-cli/releases/{version}/{platform}/droid";
  const droidHashes = await calculatePlatformHashes(droidUrlTemplate, platforms, {
    version: latest,
  });

  const ripgrepUrlTemplate = "https://downloads.factory.ai/ripgrep/{platform}/rg";
  const ripgrepHashes = await calculatePlatformHashes(ripgrepUrlTemplate, platforms);

  const nextData: Record<string, JsonValue> = {
    version: latest,
    droid: droidHashes,
    ripgrep: ripgrepHashes,
  };
  await writeJsonFile(hashesFilePath, nextData);

  console.log(`Updated to ${latest}`);
}

if (import.meta.main) {
  await main();
}
