import { calculateUrlHash } from "./hash.ts";
import { pMap } from "./pMap.ts";
import { formatTemplate } from "./template.ts";

export async function calculatePlatformHashes(
  urlTemplate: string,
  platforms: Readonly<Record<string, string>>,
  formatArgs: Readonly<Record<string, string>> = {},
): Promise<Record<string, string>> {
  const entries = Object.entries(platforms).sort(([a], [b]) => a.localeCompare(b));

  const results = await pMap(
    entries,
    async ([nixPlatform, platformValue]) => {
      const url = formatTemplate(urlTemplate, { ...formatArgs, platform: platformValue });
      const hash = await calculateUrlHash(url);
      console.log(`Fetched hash for ${nixPlatform}`);
      return { nixPlatform, hash };
    },
    { concurrency: entries.length },
  );

  const hashes: Record<string, string> = {};
  for (const { nixPlatform, hash } of results) {
    hashes[nixPlatform] = hash;
  }
  return hashes;
}
