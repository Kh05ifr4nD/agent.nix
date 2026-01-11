import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { assertRecord, assertString } from "./updater/assert.ts";
import { runCaptureChecked } from "./updater/command.ts";

type PackageMetadata = Readonly<{
  description: string;
  version: string;
  license: string;
  homepage: string | null;
  sourceType: string;
  hideFromDocs: boolean;
  hasMainProgram: boolean;
  category: string;
}>;

const beginMarker = "<!-- BEGIN GENERATED PACKAGE DOCS -->";
const endMarker = "<!-- END GENERATED PACKAGE DOCS -->";

const categoryOrder = [
  "AI Coding Agents",
  "Codex Ecosystem",
  "Workflow & Project Management",
  "Code Review",
  "Utilities",
  "Uncategorized",
] as const;

function parsePackageMetadata(value: unknown, context: string): PackageMetadata {
  assertRecord(value, `${context}: expected object`);

  const description = value["description"];
  const version = value["version"];
  const license = value["license"];
  const homepage = value["homepage"];
  const sourceType = value["sourceType"];
  const hideFromDocs = value["hideFromDocs"];
  const hasMainProgram = value["hasMainProgram"];
  const category = value["category"];

  assertString(description, `${context}: description`);
  assertString(version, `${context}: version`);
  assertString(license, `${context}: license`);
  if (homepage !== null) assertString(homepage, `${context}: homepage`);
  assertString(sourceType, `${context}: sourceType`);
  if (typeof hideFromDocs !== "boolean") {
    throw new Error(`${context}: hideFromDocs must be a boolean`);
  }
  if (typeof hasMainProgram !== "boolean") {
    throw new Error(`${context}: hasMainProgram must be a boolean`);
  }
  assertString(category, `${context}: category`);

  return {
    description,
    version,
    license,
    homepage,
    sourceType,
    hideFromDocs,
    hasMainProgram,
    category,
  };
}

function parseAllPackagesMetadata(value: unknown): Record<string, PackageMetadata> {
  assertRecord(value, "nix eval output");

  const result: Record<string, PackageMetadata> = {};
  for (const [pkg, metaOrNull] of Object.entries(value)) {
    if (metaOrNull === null) continue;
    result[pkg] = parsePackageMetadata(metaOrNull, `metadata[${pkg}]`);
  }

  return result;
}

export async function getFlakeRef(): Promise<string> {
  const override = Deno.env.get("PACKAGE_DOCS_FLAKE");
  if (override) return override;

  const githubRepo = Deno.env.get("GITHUB_REPOSITORY");
  if (githubRepo) return `github:${githubRepo}`;

  const output = await runCaptureChecked("git", ["remote", "get-url", "origin"]);
  const url = output.stdout.trim();

  if (!url.includes("github.com")) return ".";

  const match = url.match(/[:/]([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+?)(?:\.git)?$/);
  if (!match) return ".";

  const owner = match[1];
  const repo = match[2];
  return `github:${owner}/${repo}`;
}

export async function getAllPackagesMetadata(): Promise<Record<string, PackageMetadata>> {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const nixFile = join(scriptDir, "generatePackageDocs.nix");

  const output = await runCaptureChecked("nix", [
    "--accept-flake-config",
    "eval",
    "--json",
    "--file",
    nixFile,
  ]);

  const parsed: unknown = JSON.parse(output.stdout);
  return parseAllPackagesMetadata(parsed);
}

function generatePackageDoc(
  packageName: string,
  metadata: PackageMetadata,
  flakeRef: string,
): string {
  const lines: string[] = [];

  lines.push("<details>");
  lines.push(`<summary><strong>${packageName}</strong> - ${metadata.description}</summary>`);
  lines.push("");
  lines.push(`- **Source**: ${metadata.sourceType}`);
  lines.push(`- **License**: ${metadata.license}`);

  if (metadata.homepage) {
    lines.push(`- **Homepage**: ${metadata.homepage}`);
  }

  lines.push(`- **Usage**: \`nix run ${flakeRef}#${packageName} -- --help\``);
  lines.push(
    `- **Nix**: [packages/${packageName}/package.nix](packages/${packageName}/package.nix)`,
  );

  const packageReadme = `packages/${packageName}/README.md`;
  try {
    Deno.statSync(packageReadme);
    lines.push(
      `- **Documentation**: See [${packageReadme}](${packageReadme}) for detailed usage`,
    );
  } catch {
    // no-op
  }

  lines.push("");
  lines.push("</details>");
  return lines.join("\n");
}

function generateAllDocs(
  metadataByPackage: Record<string, PackageMetadata>,
  flakeRef: string,
): string {
  const byCategory = new Map<string, Array<[string, PackageMetadata]>>();

  const entries = Object.entries(metadataByPackage).sort(([a], [b]) => a.localeCompare(b));
  for (const [packageName, meta] of entries) {
    const category = meta.category;
    const list = byCategory.get(category) ?? [];
    list.push([packageName, meta]);
    byCategory.set(category, list);
  }

  const docs: string[] = [];

  const seen = new Set<string>();
  for (const category of categoryOrder) {
    const entries = byCategory.get(category);
    if (!entries) continue;
    seen.add(category);
    docs.push(`### ${category}\n`);
    for (const [packageName, meta] of entries) {
      docs.push(generatePackageDoc(packageName, meta, flakeRef));
    }
    docs.push("");
  }

  const remainingCategories = [...byCategory.keys()].filter((c) => !seen.has(c)).sort();
  for (const category of remainingCategories) {
    const entries = byCategory.get(category);
    if (!entries) continue;
    docs.push(`### ${category}\n`);
    for (const [packageName, meta] of entries) {
      docs.push(generatePackageDoc(packageName, meta, flakeRef));
    }
    docs.push("");
  }

  return docs.join("\n").trimEnd();
}

export async function updateReadme(readmePath: string): Promise<boolean> {
  const content = await Deno.readTextFile(readmePath);

  const beginIndex = content.indexOf(beginMarker);
  const endIndex = content.indexOf(endMarker);

  if (beginIndex === -1 || endIndex === -1) {
    throw new Error(`Could not find markers in ${readmePath}`);
  }
  if (endIndex < beginIndex) {
    throw new Error(`END marker appears before BEGIN marker in ${readmePath}`);
  }

  const flakeRef = await getFlakeRef();
  const metadata = await getAllPackagesMetadata();
  const generated = generateAllDocs(metadata, flakeRef);

  const newContent = content.slice(0, beginIndex + beginMarker.length) +
    "\n\n" +
    generated +
    "\n" +
    content.slice(endIndex);

  if (newContent === content) return false;

  await Deno.writeTextFile(readmePath, newContent);
  return true;
}

async function main(): Promise<void> {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const readmePath = join(scriptDir, "..", "README.md");

  const modified = await updateReadme(readmePath);
  console.log(modified ? `Updated ${readmePath}` : `No changes to ${readmePath}`);
}

if (import.meta.main) {
  await main();
}
