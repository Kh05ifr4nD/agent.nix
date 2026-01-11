import { assertString } from "./assert.ts";
import { dummySha256Hash, extractHashFromBuildError } from "./hash.ts";
import { type JsonValue, writeJsonFile } from "./jsonFile.ts";
import { nixBuildCapture } from "./nix.ts";

export async function calculateDependencyHash(
  packageAttr: string,
  hashKey: string,
  hashesFilePath: string,
  data: Readonly<Record<string, JsonValue>>,
): Promise<string> {
  const original = data[hashKey];
  assertString(original, `${hashesFilePath}: ${hashKey} must be a string`);

  const dummyData: Record<string, JsonValue> = { ...data, [hashKey]: dummySha256Hash };
  await writeJsonFile(hashesFilePath, dummyData);

  try {
    const output = await nixBuildCapture(packageAttr);
    if (output.code === 0) {
      throw new Error(`Build succeeded with dummy ${hashKey} - expected failure`);
    }

    const combined = `${output.stdout}\n${output.stderr}`;
    const extracted = extractHashFromBuildError(combined);
    if (!extracted) {
      throw new Error(`Could not extract ${hashKey} from nix build error output`);
    }
    return extracted;
  } catch (err) {
    const restored: Record<string, JsonValue> = { ...data, [hashKey]: original };
    await writeJsonFile(hashesFilePath, restored);
    throw err;
  }
}
