type MatrixItemType = "package" | "flake-input";

type MatrixItem = {
  type: MatrixItemType;
  name: string;
  current_version: string;
};

type Matrix = {
  include: MatrixItem[];
};

function getEnv(name: string, fallback = ""): string {
  return Deno.env.get(name) ?? fallback;
}

async function runCapture(
  command: string,
  args: string[],
  opts: { env?: Record<string, string> } = {},
): Promise<{ code: number; stdout: string; stderr: string }> {
  const output = await new Deno.Command(command, {
    args,
    stdout: "piped",
    stderr: "piped",
    env: opts.env,
  }).output();

  const decoder = new TextDecoder();
  return {
    code: output.code,
    stdout: decoder.decode(output.stdout),
    stderr: decoder.decode(output.stderr),
  };
}

async function discoverPackages(
  packagesFilter: string | undefined,
  system: string,
): Promise<MatrixItem[]> {
  const items: MatrixItem[] = [];

  console.log("Discovering packages...");

  const config = JSON.stringify({
    system,
    filter: packagesFilter ? packagesFilter.split(/\s+/).filter(Boolean) : null,
  });

  const expr = `
    let
      config = builtins.fromJSON (builtins.getEnv "DISCOVERY_CONFIG");
      flake = builtins.getFlake (toString ./.);
      pkgs = flake.packages.\${config.system};
      getVersion = name:
        if pkgs ? \${name} && pkgs.\${name} ? version
        then { inherit name; value = pkgs.\${name}.version; }
        else null;
    in
      if config.filter == null then
        builtins.mapAttrs (name: pkg:
          if pkg ? version then pkg.version else null
        ) pkgs
      else
        builtins.listToAttrs
          (builtins.filter (x: x != null) (map getVersion config.filter))
  `;

  const env = {
    ...Deno.env.toObject(),
    DISCOVERY_CONFIG: config,
  };

  const result = await runCapture("nix", ["eval", "--json", "--impure", "--expr", expr], {
    env,
  });

  if (result.code !== 0) {
    console.error(`Failed to evaluate packages:\n${result.stderr}`);
    return items;
  }

  const versions: Record<string, string | null> = JSON.parse(result.stdout);
  const names = Object.keys(versions).sort();

  for (const name of names) {
    const version = versions[name];
    if (version !== null) {
      items.push({ type: "package", name, current_version: version });
    } else if (!packagesFilter) {
      console.log(`Skipping ${name} (no version attribute)`);
    }
  }

  if (packagesFilter) {
    const found = new Set(Object.keys(versions));
    for (const pkg of packagesFilter.split(/\s+/).filter(Boolean)) {
      if (!found.has(pkg)) {
        console.log(`Warning: Package ${pkg} not found or has no version`);
      }
    }
  }

  return items;
}

async function discoverFlakeInputs(inputsFilter: string | undefined): Promise<MatrixItem[]> {
  const items: MatrixItem[] = [];

  console.log("Discovering flake inputs...");

  try {
    await Deno.stat("flake.lock");
  } catch {
    console.log("No flake.lock found, skipping input updates");
    return items;
  }

  const lockData = JSON.parse(await Deno.readTextFile("flake.lock"));
  const nodes: Record<string, unknown> = lockData?.nodes ?? {};

  const inputNames = inputsFilter
    ? inputsFilter.split(/\s+/).filter(Boolean)
    : Object.keys(nodes).filter((k) => k !== "root").sort();

  for (const inputName of inputNames) {
    const node = nodes[inputName] as { locked?: { rev?: string } } | undefined;
    if (!node) continue;
    const rev = (node.locked?.rev ?? "unknown").slice(0, 8);
    items.push({ type: "flake-input", name: inputName, current_version: rev });
  }

  return items;
}

async function appendGithubOutput(line: string): Promise<void> {
  const githubOutput = getEnv("GITHUB_OUTPUT");
  if (!githubOutput) return;
  await Deno.writeTextFile(githubOutput, `${line}\n`, { append: true });
}

async function main(): Promise<void> {
  const packages = getEnv("PACKAGES").trim();
  const inputs = getEnv("INPUTS").trim();
  const system = getEnv("SYSTEM", "x86_64-linux").trim();

  console.log("=== Discovery Configuration ===");
  console.log(`PACKAGES: ${packages || "<all>"}`);
  console.log(`INPUTS: ${inputs || "<all>"}`);
  console.log(`SYSTEM: ${system}`);
  console.log();

  const matrixItems: MatrixItem[] = [];
  matrixItems.push(...await discoverPackages(packages || undefined, system));
  matrixItems.push(...await discoverFlakeInputs(inputs || undefined));

  console.log();
  console.log("=== Discovery Results ===");

  let matrix: Matrix;
  let hasItems: boolean;

  if (matrixItems.length === 0) {
    matrix = { include: [] };
    hasItems = false;
    console.log("No items to update");
  } else {
    matrix = { include: matrixItems };
    hasItems = true;
    console.log(`Found ${matrixItems.length} item(s) to update`);
  }

  const matrixJson = JSON.stringify(matrix);

  await appendGithubOutput(`matrix=${matrixJson}`);
  await appendGithubOutput(`has_items=${String(hasItems)}`);

  if (!getEnv("GITHUB_OUTPUT")) {
    console.log();
    console.log("=== GitHub Actions Output Format ===");
    console.log(`matrix=${matrixJson}`);
    console.log(`has_items=${String(hasItems)}`);
  }
}

if (import.meta.main) {
  await main();
}

