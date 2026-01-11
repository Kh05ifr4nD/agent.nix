import { assertArray, assertRecord } from "../../scripts/updater/assert.ts";
import {
  type Env,
  runCapture,
  runCaptureChecked,
  runChecked,
  runStatus,
  trimLines,
} from "../../scripts/updater/command.ts";
import { fileExists } from "../../scripts/updater/fs.ts";
import { updateReadme } from "../../scripts/generatePackageDocs.ts";

type UpdateType = "package" | "flake-input";

function getEnv(name: string, fallback = ""): string {
  return Deno.env.get(name) ?? fallback;
}

function hasEnv(name: string): boolean {
  return Deno.env.has(name);
}

async function readSmokePackages(): Promise<string> {
  if (hasEnv("SMOKE_PACKAGES")) {
    return getEnv("SMOKE_PACKAGES");
  }

  const file = ".github/smokePackages.txt";
  if (!(await fileExists(file))) return "";

  const content = await Deno.readTextFile(file);
  return content
    .split(/\r?\n/)
    .map((line) => line.replace(/#.*$/, "").trim())
    .filter(Boolean)
    .join(" ");
}

function splitLabels(labels: string): string[] {
  return labels
    .split(",")
    .map((l) => l.trim())
    .filter(Boolean);
}

async function gitPorcelain(env: Env): Promise<string> {
  const result = await runCaptureChecked("git", ["status", "--porcelain"], { env });
  return result.stdout;
}

async function nixEvalPackageVersion(
  name: string,
  system: string,
  env: Env,
): Promise<string> {
  const attr = `.#packages.${system}."${name}".version`;
  const result = await runCapture("nix", ["eval", "--raw", "--impure", attr], { env });
  if (result.code !== 0) return "unknown";
  return result.stdout.trim() || "unknown";
}

async function readFlakeInputRev(name: string): Promise<string> {
  const lockText = await Deno.readTextFile("flake.lock");
  const lockData: unknown = JSON.parse(lockText);

  assertRecord(lockData, "flake.lock: expected object");
  const nodes = lockData["nodes"];
  assertRecord(nodes, "flake.lock.nodes: expected object");

  const node = nodes[name];
  if (node === null || node === undefined) return "unknown";
  assertRecord(node, `flake.lock.nodes.${name}: expected object`);

  const locked = node["locked"];
  if (locked === null || locked === undefined) return "unknown";
  assertRecord(locked, `flake.lock.nodes.${name}.locked: expected object`);

  const rev = locked["rev"];
  if (typeof rev !== "string" || !rev) return "unknown";
  return rev.slice(0, 8);
}

async function ghPrNumberForBranch(
  branch: string,
  env: Env,
): Promise<number | null> {
  const result = await runCaptureChecked(
    "gh",
    ["pr", "list", "--head", branch, "--json", "number"],
    {
      env,
    },
  );

  const parsed: unknown = JSON.parse(result.stdout);
  assertArray(parsed, "gh pr list: expected array");
  if (parsed.length === 0) return null;

  const first = parsed[0];
  assertRecord(first, "gh pr list[0]: expected object");
  const number = first["number"];
  return typeof number === "number" ? number : null;
}

async function main(): Promise<void> {
  const [typeArg, name, currentVersion] = Deno.args;
  if (!typeArg || !name || !currentVersion) {
    throw new Error("Usage: updateItem.ts <package|flake-input> <name> <currentVersion>");
  }
  const type = typeArg as UpdateType;
  if (type !== "package" && type !== "flake-input") {
    throw new Error(`Unknown type '${typeArg}' (expected 'package' or 'flake-input')`);
  }

  const system = getEnv("SYSTEM", "x86_64-linux");
  const prLabels = getEnv("PR_LABELS", "dependencies,automated");
  const autoMerge = getEnv("AUTO_MERGE", "false");

  const ghToken = getEnv("GH_TOKEN");
  if (!ghToken) {
    console.error("Error: GH_TOKEN is not set");
    Deno.exit(1);
  }

  const env = {
    ...Deno.env.toObject(),
    NIX_PATH: "nixpkgs=flake:nixpkgs",
  };

  const status = await gitPorcelain(env);
  if (status.trim()) {
    console.error("Error: working tree is not clean before update");
    console.error(status.trimEnd());
    Deno.exit(1);
  }

  console.log("=== Update target ===");
  console.log(`type=${type}`);
  console.log(`name=${name}`);
  console.log(`system=${system}`);
  console.log(`currentVersion=${currentVersion}`);
  console.log();

  if (type === "package") {
    const updaterPath = `packages/${name}/update.ts`;
    if (await fileExists(updaterPath)) {
      console.log(`Running ${updaterPath}`);
      await runChecked(
        "deno",
        [
          "run",
          "--config",
          "deno.jsonc",
          "--allow-run",
          "--allow-read",
          "--allow-write",
          "--allow-env",
          "--allow-net",
          updaterPath,
        ],
        { env },
      );
    } else {
      console.log(`No update.ts for ${name}; running nix-update`);
      const argsPathCandidates = [
        `packages/${name}/nixUpdateArgs`,
        `packages/${name}/nix-update-args`,
      ];

      let extraArgs: string[] = [];
      for (const argsPath of argsPathCandidates) {
        if (!(await fileExists(argsPath))) continue;
        extraArgs = (await Deno.readTextFile(argsPath))
          .split(/\r?\n/)
          .map((l) => l.replace(/#.*$/, "").trim())
          .filter(Boolean);
        break;
      }

      await runChecked("nix-update", ["--flake", name, ...extraArgs], { env });
    }
  } else {
    console.log(`Running nix flake update ${name}`);
    await runChecked("nix", ["flake", "update", name], { env });
  }

  {
    const diff = await runStatus("git", ["diff", "--quiet"], { env });
    if (diff === 0) {
      console.log("No changes detected; skipping PR.");
      return;
    }
  }

  console.log("Regenerating README package docs (if needed)...");
  await updateReadme("README.md");

  console.log("Formatting repository...");
  await runChecked("nix", ["fmt"], { env });

  {
    const diff = await runStatus("git", ["diff", "--quiet"], { env });
    if (diff === 0) {
      console.log("No changes detected after formatting; skipping PR.");
      return;
    }
  }

  let newVersion = "unknown";
  if (type === "package") {
    newVersion = await nixEvalPackageVersion(name, system, env);
  } else {
    newVersion = await readFlakeInputRev(name);
  }

  console.log("=== Validation ===");
  if (type === "package") {
    await runChecked("nix", [
      "build",
      "--accept-flake-config",
      "--no-link",
      `.#checks.${system}.pkgs-${name}`,
    ], {
      env,
    });
    await runChecked("nix", [
      "build",
      "--accept-flake-config",
      "--no-link",
      `.#checks.${system}.pkgs-formatter-check`,
    ], {
      env,
    });
    await runChecked("nix", [
      "build",
      "--accept-flake-config",
      "--no-link",
      `.#checks.${system}.pkgs-formatter-denoCheck`,
    ], {
      env,
    });
  } else {
    await runChecked("nix", ["flake", "check", "--no-build", "--accept-flake-config"], { env });
    await runChecked("nix", [
      "build",
      "--accept-flake-config",
      "--no-link",
      `.#checks.${system}.pkgs-formatter-check`,
    ], {
      env,
    });
    await runChecked("nix", [
      "build",
      "--accept-flake-config",
      "--no-link",
      `.#checks.${system}.pkgs-formatter-denoCheck`,
    ], {
      env,
    });

    const smokePackages = (await readSmokePackages()).trim();
    if (smokePackages) {
      console.log("=== Smoke build (flake input update) ===");
      console.log(smokePackages);
      for (const pkg of smokePackages.split(/\s+/).filter(Boolean)) {
        await runChecked("nix", [
          "build",
          "--accept-flake-config",
          "--no-link",
          `.#checks.${system}.pkgs-${pkg}`,
        ], {
          env,
        });
      }
    }
  }

  const changedFiles = trimLines(
    (await runCapture("git", ["diff", "--name-only"], { env })).stdout,
  );
  const untrackedFiles = trimLines(
    (await runCapture("git", ["ls-files", "--others", "--exclude-standard"], { env })).stdout,
  );
  const allFiles = Array.from(new Set([...changedFiles, ...untrackedFiles])).sort();

  if (allFiles.length === 0) {
    console.error("Error: expected changes but working tree is clean");
    Deno.exit(1);
  }

  console.log("=== Worktree changes ===");
  console.log(allFiles.join("\n"));
  console.log();

  const isAllowedChange = (file: string): boolean => {
    if (type === "package") {
      if (file === "README.md") return true;
      return file.startsWith(`packages/${name}/`);
    }

    return file === "flake.lock" || file === "README.md";
  };

  for (const file of allFiles) {
    if (!isAllowedChange(file)) {
      console.error(`Error: unexpected change outside allowed scope: ${file}`);
      console.error(
        `Hint: package updates must only touch packages/${name}/** and optionally README.md`,
      );
      console.error(
        "Hint: flake-input updates must only touch flake.lock and optionally README.md",
      );
      Deno.exit(1);
    }
  }

  const branch = type === "package" ? `update/${name}` : `update/flake-input/${name}`;
  const prTitle = type === "package"
    ? `${name}: ${currentVersion} -> ${newVersion}`
    : `flake.lock: Update ${name}`;
  const prBody = type === "package"
    ? `Automated update of ${name} from ${currentVersion} to ${newVersion}.`
    : `This PR updates the flake input \`${name}\`.\n\n- ${name}: \`${currentVersion}\` → \`${newVersion}\``;

  console.log("=== Create/Update PR ===");
  console.log(`branch=${branch}`);
  console.log(`title=${prTitle}`);
  console.log();

  await runChecked("git", ["switch", "-C", branch], { env });

  if (type === "package") {
    await runChecked("git", ["add", `packages/${name}`, "README.md"], { env });
  } else {
    await runChecked("git", ["add", "flake.lock", "README.md"], { env });
  }

  {
    const staged = await runStatus("git", ["diff", "--cached", "--quiet"], { env });
    if (staged === 0) {
      console.error("Error: nothing staged for commit");
      Deno.exit(1);
    }
  }

  await runChecked("git", ["commit", "-m", prTitle, "--signoff"], { env });
  await runChecked("git", ["push", "--force", "--set-upstream", "origin", branch], { env });

  const labelArgs = splitLabels(prLabels).flatMap((label) => ["--label", label]);

  let prNumber = await ghPrNumberForBranch(branch, env);
  if (prNumber !== null) {
    console.log(`Updating existing PR #${prNumber}`);
    await runChecked("gh", [
      "pr",
      "edit",
      String(prNumber),
      "--title",
      prTitle,
      "--body",
      prBody,
      ...labelArgs,
    ], {
      env,
    });
  } else {
    console.log("Creating new PR");
    await runChecked("gh", [
      "pr",
      "create",
      "--title",
      prTitle,
      "--body",
      prBody,
      "--base",
      "main",
      "--head",
      branch,
      ...labelArgs,
    ], {
      env,
    });
    prNumber = await ghPrNumberForBranch(branch, env);
  }

  if (autoMerge === "true" && prNumber !== null) {
    console.log(`Enabling auto-merge for PR #${prNumber}`);
    try {
      await runChecked("gh", ["pr", "merge", String(prNumber), "--auto", "--squash"], { env });
    } catch {
      console.log("Note: auto-merge may require branch protection rules");
    }
  }
}

if (import.meta.main) {
  try {
    await main();
  } catch (err) {
    console.error(err instanceof Error ? err.message : String(err));
    Deno.exit(1);
  }
}
