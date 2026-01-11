export type Env = Readonly<Record<string, string>>;

export type RunOptions = Readonly<{
  cwd?: string;
  env?: Env;
}>;

export type CapturedOutput = Readonly<{
  code: number;
  stdout: string;
  stderr: string;
}>;

export class CommandFailedError extends Error {
  readonly command: string;
  readonly args: readonly string[];
  readonly code: number;
  readonly stdout: string;
  readonly stderr: string;

  constructor(command: string, args: readonly string[], output: CapturedOutput) {
    const joinedArgs = args.map((a) => JSON.stringify(a)).join(" ");
    const message = [
      `Command failed (${output.code}): ${command} ${joinedArgs}`,
      output.stdout.trim() ? `--- stdout ---\n${output.stdout.trimEnd()}` : "",
      output.stderr.trim() ? `--- stderr ---\n${output.stderr.trimEnd()}` : "",
    ]
      .filter(Boolean)
      .join("\n");

    super(message);
    this.name = "CommandFailedError";
    this.command = command;
    this.args = args;
    this.code = output.code;
    this.stdout = output.stdout;
    this.stderr = output.stderr;
  }
}

export function trimLines(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

export async function runStatus(
  command: string,
  args: readonly string[],
  opts: RunOptions = {},
): Promise<number> {
  const commandOptions: Deno.CommandOptions = {
    args: [...args],
    stdout: "inherit",
    stderr: "inherit",
    stdin: "inherit",
    ...(opts.env !== undefined ? { env: { ...opts.env } } : {}),
    ...(opts.cwd !== undefined ? { cwd: opts.cwd } : {}),
  };

  const status = await new Deno.Command(command, commandOptions).spawn().status;

  return status.code;
}

export async function runChecked(
  command: string,
  args: readonly string[],
  opts: RunOptions = {},
): Promise<void> {
  const code = await runStatus(command, args, opts);
  if (code !== 0) {
    throw new Error(
      `Command failed (${code}): ${command} ${args.map((a) => JSON.stringify(a)).join(" ")}`,
    );
  }
}

export async function runCapture(
  command: string,
  args: readonly string[],
  opts: RunOptions = {},
): Promise<CapturedOutput> {
  const commandOptions: Deno.CommandOptions = {
    args: [...args],
    stdout: "piped",
    stderr: "piped",
    ...(opts.env !== undefined ? { env: { ...opts.env } } : {}),
    ...(opts.cwd !== undefined ? { cwd: opts.cwd } : {}),
  };

  const output = await new Deno.Command(command, commandOptions).output();

  const decoder = new TextDecoder();
  return {
    code: output.code,
    stdout: decoder.decode(output.stdout),
    stderr: decoder.decode(output.stderr),
  };
}

export async function runCaptureChecked(
  command: string,
  args: readonly string[],
  opts: RunOptions = {},
): Promise<CapturedOutput> {
  const output = await runCapture(command, args, opts);
  if (output.code !== 0) {
    throw new CommandFailedError(command, args, output);
  }
  return output;
}
