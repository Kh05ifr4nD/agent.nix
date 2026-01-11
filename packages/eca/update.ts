import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { assertString } from "../../scripts/updater/assert.ts";
import {
  calculatePlatformHashes,
  calculateUrlHash,
  fetchGithubLatestRelease,
  readJsonObjectFile,
  shouldUpdate,
  writeJsonFile,
} from "../../scripts/updater/mod.ts";
import type { JsonValue } from "../../scripts/updater/mod.ts";

const platforms = {
  "x86_64-linux": "linux-amd64",
  "aarch64-linux": "linux-aarch64",
  "x86_64-darwin": "macos-amd64",
  "aarch64-darwin": "macos-aarch64",
} as const satisfies Record<string, string>;

async function main(): Promise<void> {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const hashesFilePath = join(scriptDir, "hashes.json");

  const data = await readJsonObjectFile(hashesFilePath);
  const current = data["version"];
  assertString(current, `${hashesFilePath}: version must be a string`);

  const latest = await fetchGithubLatestRelease("editor-code-assistant", "eca");

  console.log(`Current: ${current}, Latest: ${latest}`);

  if (!shouldUpdate(current, latest)) {
    console.log("Already up to date");
    return;
  }

  console.log(`Updating eca from ${current} to ${latest}`);

  const urlTemplate =
    "https://github.com/editor-code-assistant/eca/releases/download/{version}/eca-native-{platform}.zip";
  const nativeHashes = await calculatePlatformHashes(urlTemplate, platforms, { version: latest });

  const jarUrl = `https://github.com/editor-code-assistant/eca/releases/download/${latest}/eca.jar`;
  console.log("Fetching hash for JAR...");
  const jarHash = await calculateUrlHash(jarUrl);

  const nextData: Record<string, JsonValue> = {
    version: latest,
    ...nativeHashes,
    jar: jarHash,
  };
  await writeJsonFile(hashesFilePath, nextData);

  console.log(`Updated to ${latest}`);
}

if (import.meta.main) {
  await main();
}
