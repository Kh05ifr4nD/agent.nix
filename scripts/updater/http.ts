export type FetchOptions = Readonly<{
  headers?: HeadersInit;
  timeoutMs?: number;
}>;

async function fetchWithTimeout(url: string, opts: FetchOptions): Promise<Response> {
  const timeoutMs = opts.timeoutMs ?? 30_000;
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, {
      headers: opts.headers,
      signal: controller.signal,
    });
  } finally {
    clearTimeout(id);
  }
}

export async function fetchText(url: string, opts: FetchOptions = {}): Promise<string> {
  const res = await fetchWithTimeout(url, opts);
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`HTTP ${res.status} ${res.statusText} for ${url}${body ? `\n${body}` : ""}`);
  }
  return await res.text();
}

export async function fetchJson(url: string, opts: FetchOptions = {}): Promise<unknown> {
  const text = await fetchText(url, opts);
  const parsed: unknown = JSON.parse(text);
  return parsed;
}
