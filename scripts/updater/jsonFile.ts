import { assert, assertRecord, isRecord } from "./assert.ts";

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

function isJsonValue(value: unknown): value is JsonValue {
  if (
    value === null ||
    typeof value === "boolean" ||
    typeof value === "number" ||
    typeof value === "string"
  ) {
    return true;
  }

  if (Array.isArray(value)) {
    return value.every(isJsonValue);
  }

  if (isRecord(value)) {
    return Object.values(value).every(isJsonValue);
  }

  return false;
}

export async function readJsonValueFile(path: string): Promise<JsonValue> {
  const text = await Deno.readTextFile(path);
  const parsed: unknown = JSON.parse(text);
  assert(isJsonValue(parsed), `${path}: expected JSON value`);
  return parsed;
}

export async function readJsonObjectFile(path: string): Promise<Record<string, JsonValue>> {
  const value = await readJsonValueFile(path);
  assertRecord(value, `${path}: expected JSON object`);
  return value;
}

export async function writeJsonFile(path: string, value: JsonValue): Promise<void> {
  assert(isJsonValue(value), `${path}: expected JSON value`);
  await Deno.writeTextFile(path, `${JSON.stringify(value, null, 2)}\n`);
}
