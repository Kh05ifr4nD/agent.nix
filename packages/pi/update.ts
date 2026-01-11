import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { assertString } from "../../scripts/updater/assert.ts";
import {
  calculateDependencyHash,
  calculateUrlHash,
  dummySha256Hash,
  extractOrGenerateLockfile,
  fetchNpmVersion,
  readJsonObjectFile,
  shouldUpdate,
  writeJsonFile,
} from "../../scripts/updater/mod.ts";
import type { JsonValue } from "../../scripts/updater/mod.ts";

const npmPackage = "@mariozechner/pi-coding-agent";

async function main(): Promise<void> {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const hashesFilePath = join(scriptDir, "hashes.json");
  const packageLockPath = join(scriptDir, "package-lock.json");

  const currentData = await readJsonObjectFile(hashesFilePath);
  const current = currentData["version"];
  assertString(current, `${hashesFilePath}: version must be a string`);

  const latest = await fetchNpmVersion(npmPackage);

  console.log(`Current: ${current}, Latest: ${latest}`);

  if (!shouldUpdate(current, latest)) {
    console.log("Already up to date");
    return;
  }

  const tarballUrl = `https://registry.npmjs.org/${npmPackage}/-/pi-coding-agent-${latest}.tgz`;

  console.log("Calculating source hash...");
  const sourceHash = await calculateUrlHash(tarballUrl);

  await extractOrGenerateLockfile(tarballUrl, packageLockPath);

  const nextData: Record<string, JsonValue> = {
    version: latest,
    sourceHash,
    npmDepsHash: dummySha256Hash,
  };
  await writeJsonFile(hashesFilePath, nextData);

  try {
    const npmDepsHash = await calculateDependencyHash(
      ".#pi",
      "npmDepsHash",
      hashesFilePath,
      nextData,
    );
    const finalData: Record<string, JsonValue> = { ...nextData, npmDepsHash };
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
