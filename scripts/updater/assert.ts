export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function assert(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

export function assertRecord(
  value: unknown,
  context: string,
): asserts value is Record<string, unknown> {
  assert(isRecord(value), `${context}: expected object`);
}

export function assertString(value: unknown, context: string): asserts value is string {
  assert(typeof value === "string", `${context}: expected string`);
}

export function assertNumber(value: unknown, context: string): asserts value is number {
  assert(typeof value === "number", `${context}: expected number`);
}

export function assertArray(value: unknown, context: string): asserts value is unknown[] {
  assert(Array.isArray(value), `${context}: expected array`);
}

export function assertNever(value: never, message = "Unexpected value"): never {
  throw new Error(`${message}: ${String(value)}`);
}
