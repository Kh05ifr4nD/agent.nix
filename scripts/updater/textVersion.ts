import { fetchText } from "./http.ts";

export async function fetchVersionFromText(url: string, pattern: string): Promise<string> {
  const text = await fetchText(url);
  const regex = new RegExp(pattern);
  const match = text.match(regex);
  const version = match?.[1];
  if (!version) {
    throw new Error(`Could not extract version from ${url} using pattern ${pattern}`);
  }
  return version;
}
