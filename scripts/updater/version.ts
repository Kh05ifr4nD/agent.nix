export type ParsedVersion = Readonly<{
  numericParts: readonly number[] | null;
  suffix: string;
}>;

function stripLeadingV(version: string): string {
  return version.startsWith("v") ? version.slice(1) : version;
}

export function parseVersion(version: string): ParsedVersion {
  const normalized = stripLeadingV(version);

  const parts = normalized.replace("+", "-").split("-", 2);
  const numericStr = parts[0];
  const suffix = parts[1] ?? "";

  if (!numericStr) {
    return { numericParts: null, suffix };
  }

  const numericParts = numericStr
    .split(".")
    .map((part) => {
      const n = Number(part);
      return Number.isInteger(n) ? n : NaN;
    });

  const parsedNumericParts = numericParts.some(Number.isNaN) ? null : numericParts;

  return {
    numericParts: parsedNumericParts,
    suffix,
  };
}

export function compareVersions(a: string, b: string): number {
  if (a === b) return 0;

  const parsedA = parseVersion(a);
  const parsedB = parseVersion(b);

  if (parsedA.numericParts === null || parsedB.numericParts === null) {
    return a < b ? -1 : 1;
  }

  const max = Math.max(parsedA.numericParts.length, parsedB.numericParts.length);
  for (let i = 0; i < max; i += 1) {
    const aPart = parsedA.numericParts[i] ?? 0;
    const bPart = parsedB.numericParts[i] ?? 0;
    if (aPart < bPart) return -1;
    if (aPart > bPart) return 1;
  }

  if (parsedA.suffix === parsedB.suffix) return 0;
  if (!parsedA.suffix) return 1;
  if (!parsedB.suffix) return -1;
  return parsedA.suffix < parsedB.suffix ? -1 : 1;
}

export function shouldUpdate(current: string, latest: string): boolean {
  return compareVersions(current, latest) < 0;
}
