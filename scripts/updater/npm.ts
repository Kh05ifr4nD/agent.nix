import { join } from "node:path";

import { assertRecord, assertString } from "./assert.ts";
import { type Env, runChecked } from "./command.ts";
import { copyDir, ensureDir, fileExists } from "./fs.ts";
import { nixStorePrefetchFile } from "./hash.ts";
import { fetchJson } from "./http.ts";

function npmRegistryLatestUrl(packageName: string): string {
  return `https://registry.npmjs.org/${encodeURIComponent(packageName)}/latest`;
}

type NpmLatestResponse = Readonly<{
  version: string;
}>;

function parseNpmLatestResponse(data: unknown, context: string): NpmLatestResponse {
  assertRecord(data, `${context}: expected JSON object`);
  const version = data["version"];
  assertString(version, `${context}: expected version string`);
  return { version };
}

export async function fetchNpmVersion(packageName: string): Promise<string> {
  const url = npmRegistryLatestUrl(packageName);
  const data = await fetchJson(url, { headers: { "Accept": "application/json" } });
  const parsed = parseNpmLatestResponse(data, `npm latest ${packageName}`);
  return parsed.version;
}

export async function extractOrGenerateLockfile(
  tarballUrl: string,
  outputPath: string,
  opts: Readonly<{ env?: Env }> = {},
): Promise<void> {
  console.log("Extracting/generating package-lock.json from tarball...");

  const prefetch = await nixStorePrefetchFile(tarballUrl, { unpack: true, hashType: "sha256" });
  const unpackedDir = prefetch.storePath;

  const candidates = [
    join(unpackedDir, "package-lock.json"),
    join(unpackedDir, "package", "package-lock.json"),
  ];

  for (const candidate of candidates) {
    if (await fileExists(candidate)) {
      await ensureDir(join(outputPath, ".."));
      await Deno.copyFile(candidate, outputPath);
      console.log("Updated package-lock.json from tarball");
      return;
    }
  }

  console.log("No package-lock.json in tarball, generating...");

  const tempDir = await Deno.makeTempDir();
  try {
    const workDir = join(tempDir, "package");
    await copyDir(unpackedDir, workDir);

    const packageJson = join(workDir, "package.json");
    if (!(await fileExists(packageJson))) {
      throw new Error(`package.json not found in unpacked tarball: ${packageJson}`);
    }

    const env: Record<string, string> = { ...Deno.env.toObject(), ...(opts.env ?? {}) };
    env["HOME"] = tempDir;

    await runChecked(
      "npm",
      ["install", "--package-lock-only", "--ignore-scripts", "--no-audit", "--no-fund"],
      { cwd: workDir, env },
    );

    const generatedLockfile = join(workDir, "package-lock.json");
    if (!(await fileExists(generatedLockfile))) {
      throw new Error("Failed to generate package-lock.json");
    }

    await Deno.copyFile(generatedLockfile, outputPath);
    console.log("Generated package-lock.json");
  } finally {
    await Deno.remove(tempDir, { recursive: true }).catch(() => undefined);
  }
}
