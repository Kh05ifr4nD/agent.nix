import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { assertString } from "../../scripts/updater/assert.ts";
import {
  calculateDependencyHash,
  calculateUrlHash,
  dummySha256Hash,
  fetchGithubLatestRelease,
  readJsonObjectFile,
  shouldUpdate,
  writeJsonFile,
} from "../../scripts/updater/mod.ts";
import type { JsonValue } from "../../scripts/updater/mod.ts";

function parseCodexVersion(tag: string): string {
  const match = tag.match(/^rust-v(.+)$/);
  const version = match?.[1];
  if (!version) {
    throw new Error(`Unexpected tag format: ${JSON.stringify(tag)} (expected 'rust-v<version>')`);
  }
  return version;
}

async function main(): Promise<void> {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const hashesFilePath = join(scriptDir, "hashes.json");

  const currentData = await readJsonObjectFile(hashesFilePath);
  const current = currentData["version"];
  assertString(current, `${hashesFilePath}: version must be a string`);

  const latestTag = await fetchGithubLatestRelease("openai", "codex");
  const latest = parseCodexVersion(latestTag);

  console.log(`Current: ${current}, Latest: ${latest}`);

  if (!shouldUpdate(current, latest)) {
    console.log("Already up to date");
    return;
  }

  const tag = `rust-v${latest}`;
  const url = `https://github.com/openai/codex/archive/refs/tags/${tag}.tar.gz`;

  console.log("Calculating source hash...");
  const sourceHash = await calculateUrlHash(url, { unpack: true });

  const nextData: Record<string, JsonValue> = {
    version: latest,
    hash: sourceHash,
    cargoHash: dummySha256Hash,
  };
  await writeJsonFile(hashesFilePath, nextData);

  try {
    const cargoHash = await calculateDependencyHash(
      ".#codex",
      "cargoHash",
      hashesFilePath,
      nextData,
    );
    const finalData: Record<string, JsonValue> = { ...nextData, cargoHash };
    await writeJsonFile(hashesFilePath, finalData);
  } catch (err) {
    console.error(`Error: ${err instanceof Error ? err.message : String(err)}`);
    return;
  }

  console.log(`Updated to ${latest}`);
}

if (import.meta.main) {
  await main();
}
