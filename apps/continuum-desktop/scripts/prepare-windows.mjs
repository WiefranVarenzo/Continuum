import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { mkdirSync, copyFileSync } from "node:fs";

const appDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoDir = path.resolve(appDir, "../..");
const target = process.env.TAURI_ENV_TARGET_TRIPLE || "x86_64-pc-windows-msvc";

if (!/^x86_64-pc-windows-(msvc|gnu)$/.test(target)) {
  throw new Error(`Unsupported Windows target: ${target}`);
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, stdio: "inherit", shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

const cargo = process.platform === "win32" ? "cargo" : "cargo-xwin";
run(cargo, ["build", "--release", "--target", target, "-p", "continuum-mcp", "--bin", "continuum-mcp"], repoDir);
const staging = path.join(repoDir, "packaging/windows");
mkdirSync(staging, {recursive:true});
copyFileSync(path.join(repoDir, "target", target, "release/continuum-mcp.exe"), path.join(staging, "continuum-mcp.exe"));
run(process.execPath, ["scripts/prepare-windows-tools.mjs"], appDir);
if (process.platform === "win32") {
  if (process.env.npm_execpath) {
    run(process.execPath, [process.env.npm_execpath, "run", "build"], appDir);
  } else {
    // Node cannot spawn a .cmd shim directly with shell:false on Windows.
    run(process.env.ComSpec || "cmd.exe", ["/d", "/s", "/c", "npm run build"], appDir);
  }
} else {
  run("npm", ["run", "build"], appDir);
}
