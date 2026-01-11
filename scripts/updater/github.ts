import { assertRecord, assertString } from "./assert.ts";
import { fetchJson } from "./http.ts";

type GithubReleaseResponse = Readonly<{
  tagName: string;
}>;

function normalizeTag(tag: string): string {
  return tag.startsWith("v") ? tag.slice(1) : tag;
}

function parseGithubReleaseResponse(data: unknown, context: string): GithubReleaseResponse {
  assertRecord(data, `${context}: expected JSON object`);
  const tagName = data["tag_name"];
  assertString(tagName, `${context}: expected tag_name string`);
  return { tagName };
}

function buildGithubHeaders(token: string | undefined): HeadersInit {
  const headers: Record<string, string> = {
    "Accept": "application/vnd.github+json",
    "User-Agent": "agentNix-updater",
  };
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }
  return headers;
}

function resolveGithubToken(explicitToken?: string): string | undefined {
  return explicitToken ?? Deno.env.get("GH_TOKEN") ?? Deno.env.get("GITHUB_TOKEN") ?? undefined;
}

export async function fetchGithubLatestRelease(
  owner: string,
  repo: string,
  opts: Readonly<{ token?: string }> = {},
): Promise<string> {
  const token = resolveGithubToken(opts.token);
  const url = `https://api.github.com/repos/${owner}/${repo}/releases/latest`;
  const data = await fetchJson(url, { headers: buildGithubHeaders(token) });
  const parsed = parseGithubReleaseResponse(data, `GitHub latest release ${owner}/${repo}`);
  return normalizeTag(parsed.tagName);
}
