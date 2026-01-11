import { join } from "node:path";

export async function fileExists(path: string): Promise<boolean> {
  try {
    await Deno.stat(path);
    return true;
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) return false;
    throw err;
  }
}

export async function ensureDir(path: string): Promise<void> {
  await Deno.mkdir(path, { recursive: true });
}

export async function copyDir(src: string, dest: string): Promise<void> {
  await ensureDir(dest);

  for await (const entry of Deno.readDir(src)) {
    const srcPath = join(src, entry.name);
    const destPath = join(dest, entry.name);

    if (entry.isDirectory) {
      await copyDir(srcPath, destPath);
      continue;
    }

    if (entry.isSymlink) {
      const linkTarget = await Deno.readLink(srcPath);
      await Deno.symlink(linkTarget, destPath);
      continue;
    }

    if (entry.isFile) {
      await Deno.copyFile(srcPath, destPath);
      continue;
    }

    throw new Error(`copyDir: unsupported entry: ${srcPath}`);
  }
}
