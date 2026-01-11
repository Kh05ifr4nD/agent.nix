import { assertString } from "./assert.ts";

export function formatTemplate(
  template: string,
  params: Readonly<Record<string, string>>,
): string {
  return template.replaceAll(/\{([A-Za-z0-9_]+)\}/g, (_match, key: string) => {
    assertString(key, "formatTemplate: key");
    const value = params[key];
    if (value === undefined) {
      throw new Error(`Missing template param: ${key}`);
    }
    return value;
  });
}
