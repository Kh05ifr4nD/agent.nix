import { assertRecord, assertString } from "./assert.ts";
import { runCaptureChecked } from "./command.ts";

export const dummySha256Hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=" as const;

export type PrefetchResult = Readonly<{
  hash: string;
  storePath: string;
}>;

function parsePrefetchResult(data: unknown, context: string): PrefetchResult {
  assertRecord(data, `${context}: expected JSON object`);
  const hash = data["hash"];
  const storePath = data["storePath"];
  assertString(hash, `${context}: expected hash string`);
  assertString(storePath, `${context}: expected storePath string`);
  return { hash, storePath };
}

export async function nixStorePrefetchFile(
  url: string,
  opts: Readonly<{ unpack?: boolean; hashType?: string }> = {},
): Promise<PrefetchResult> {
  const args = ["store", "prefetch-file", "--json"];
  if (opts.hashType) {
    args.push("--hash-type", opts.hashType);
  }
  if (opts.unpack) {
    args.push("--unpack");
  }
  args.push(url);

  const output = await runCaptureChecked("nix", args);
  const parsed: unknown = JSON.parse(output.stdout);
  return parsePrefetchResult(parsed, "nix store prefetch-file");
}

export async function calculateUrlHash(
  url: string,
  opts: Readonly<{ unpack?: boolean }> = {},
): Promise<string> {
  const prefetchOpts: { unpack?: boolean; hashType: string } = { hashType: "sha256" };
  if (opts.unpack !== undefined) {
    prefetchOpts.unpack = opts.unpack;
  }
  const result = await nixStorePrefetchFile(url, prefetchOpts);
  return result.hash;
}

export function extractHashFromBuildError(output: string): string | null {
  const patterns = [
    /got:\s+(sha256-[A-Za-z0-9+/=]+)/,
    /got\s+(sha256-[A-Za-z0-9+/=]+)/,
    /actual:\s+(sha256-[A-Za-z0-9+/=]+)/,
  ];

  for (const pattern of patterns) {
    const match = output.match(pattern);
    if (match?.[1]) return match[1];
  }
  return null;
}
