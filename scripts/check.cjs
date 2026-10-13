// Cross-platform quality gate; fails immediately when any required check fails.
const { spawnSync } = require("node:child_process");
const path = require("node:path");
const root = path.resolve(__dirname, "..");
const backend = path.join(root, "backend");
const npm = process.platform === "win32" ? "npm.cmd" : "npm";
function run(command, args, cwd = backend) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: "inherit",
    windowsHide: true,
    shell: process.platform === "win32" && command.endsWith(".cmd"),
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
run(npm, ["run", "check"], path.join(root, "frontend"));
run(npm, ["run", "build"], path.join(root, "frontend"));
run("cargo", ["fmt", "--check"]);
run("cargo", ["clippy", "--locked", "--all-targets", "--", "-D", "warnings"]);
run("cargo", ["test", "--locked"]);
if (process.argv.includes("--e2e")) {
  run("cargo", ["build", "--locked"]);
  run(npm, ["run", "test:e2e"], path.join(root, "frontend"));
}
