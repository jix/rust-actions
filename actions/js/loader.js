const rev = require("./rev");
const { basename, dirname } = require("path");
const { env, pid, stdout, exit } = require("process");
const { execFileSync } = require("child_process");
const fs = require("fs");

const escapeMessage = (value) =>
  String(value)
    .replace(/%/g, "%25")
    .replace(/\r/g, "%0D")
    .replace(/\n/g, "%0A");

module.exports = (filename) => {
  try {
    const repo = env.GITHUB_ACTION_REPOSITORY;
    const arch = `${env.RUNNER_OS}-${env.RUNNER_ARCH}`.toLowerCase();
    const url = rev.startsWith("https")
      ? `${rev}/${arch}-rust-actions.zst`
      : `https://github.com/${repo}/releases/download/bin-${rev}/${arch}-rust-actions.zst`;
    const cacheDir = `${env.HOME}/.cache/rust-actions`;
    const bin = `${cacheDir}/rust-actions`;

    if (!fs.existsSync(bin)) {
      fs.mkdirSync(cacheDir, { recursive: true });
      const zst = `${cacheDir}/rust-actions.${pid}.zst`;
      const tmp = `${cacheDir}/rust-actions.${pid}.tmp`;
      execFileSync("curl", ["-fsSL", url, "-o", zst], { stdio: "inherit" });
      execFileSync("zstd", ["-qd", zst, "-o", tmp], { stdio: "inherit" });
      fs.unlinkSync(zst);
      fs.chmodSync(tmp, 0o755);
      fs.renameSync(tmp, bin);
    }

    const action = basename(dirname(filename));
    const phase = basename(filename).replace(".js", "");
    try {
      execFileSync(bin, [action, phase], { stdio: "inherit" });
    } catch (e) {
      if (e.status === null) throw e;
      exit(e.status);
    }
  } catch (e) {
    stdout.write(`\n::error title=Error initializing rust-actions::${escapeMessage(e)}\n`);
    exit(1);
  }
};
