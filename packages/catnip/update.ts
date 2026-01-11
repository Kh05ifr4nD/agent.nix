import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { assertString } from "../../scripts/updater/assert.ts";
import {
  calculatePlatformHashes,
  fetchGithubLatestRelease,
  readJsonObjectFile,
  shouldUpdate,
  writeJsonFile,
} from "../../scripts/updater/mod.ts";
import type { JsonValue } from "../../scripts/updater/mod.ts";

const platforms = {
  "x86_64-linux": "linux_amd64",
  "aarch64-linux": "linux_arm64",
  "x86_64-darwin": "darwin_amd64",
  "aarch64-darwin": "darwin_arm64",
} as const satisfies Record<string, string>;

async function main(): Promise<void> {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const hashesFilePath = join(scriptDir, "hashes.json");

  const data = await readJsonObjectFile(hashesFilePath);
  const current = data["version"];
  assertString(current, `${hashesFilePath}: version must be a string`);

  const latest = await fetchGithubLatestRelease("wandb", "catnip");

  console.log(`Current: ${current}, Latest: ${latest}`);

  if (!shouldUpdate(current, latest)) {
    console.log("Already up to date");
    return;
  }

  const urlTemplate =
    `https://github.com/wandb/catnip/releases/download/v${latest}/catnip_${latest}_{platform}.tar.gz`;
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
