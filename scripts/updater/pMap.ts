export async function pMap<T, R>(
  items: readonly T[],
  mapper: (item: T, index: number) => Promise<R>,
  opts: Readonly<{ concurrency?: number }> = {},
): Promise<R[]> {
  const concurrency = opts.concurrency ?? items.length;
  if (!Number.isInteger(concurrency) || concurrency < 1) {
    throw new Error(`pMap: invalid concurrency: ${String(concurrency)}`);
  }

  const results: R[] = new Array(items.length);
  let nextIndex = 0;

  const workerCount = Math.min(concurrency, items.length);
  const workers = Array.from({ length: workerCount }, async () => {
    while (true) {
      const index = nextIndex;
      nextIndex += 1;
      if (index >= items.length) break;
      const item = items[index];
      if (item === undefined) {
        throw new Error(`pMap: missing item at index ${String(index)}`);
      }
      results[index] = await mapper(item, index);
    }
  });

  await Promise.all(workers);
  return results;
}
