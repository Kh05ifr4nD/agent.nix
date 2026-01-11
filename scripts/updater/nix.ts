import {
  type CapturedOutput,
  runCapture,
  runCaptureChecked,
  runChecked,
  type RunOptions,
} from "./command.ts";

export type NixOptions = Readonly<RunOptions & { acceptFlakeConfig?: boolean }>;

function withAcceptFlakeConfig(args: readonly string[], acceptFlakeConfig: boolean): string[] {
  return acceptFlakeConfig ? ["--accept-flake-config", ...args] : [...args];
}

export async function nixEvalRaw(
  attr: string,
  opts: Readonly<NixOptions & { impure?: boolean }> = {},
): Promise<string> {
  const args = ["eval", "--raw", ...(opts.impure ? ["--impure"] : []), attr];
  const output = await runCaptureChecked(
    "nix",
    withAcceptFlakeConfig(args, opts.acceptFlakeConfig ?? true),
    opts,
  );
  return output.stdout.trim();
}

export async function nixBuild(
  attr: string,
  opts: NixOptions = {},
): Promise<void> {
  const args = ["build", "--log-format", "bar-with-logs", attr];
  await runChecked("nix", withAcceptFlakeConfig(args, opts.acceptFlakeConfig ?? true), opts);
}

export async function nixBuildCapture(
  attr: string,
  opts: NixOptions = {},
): Promise<CapturedOutput> {
  const args = ["build", "--log-format", "bar-with-logs", attr];
  return await runCapture("nix", withAcceptFlakeConfig(args, opts.acceptFlakeConfig ?? true), opts);
}
