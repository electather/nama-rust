// Self-check for git-guard.sh: `npm --prefix .sandcastle run check`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const guard = new URL("git-guard.sh", import.meta.url).pathname;
const realGit = "/usr/bin/git";
// Fixed identity and dates, so the same steps give the same commit hashes in every repo.
const date = "2026-01-01T00:00:00Z";
const env = {
  ...process.env,
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_AUTHOR_NAME: "t",
  GIT_AUTHOR_EMAIL: "t@t",
  GIT_AUTHOR_DATE: date,
  GIT_COMMITTER_NAME: "t",
  GIT_COMMITTER_EMAIL: "t@t",
  GIT_COMMITTER_DATE: date,
};

function run(git: string, cwd: string, args: string[]) {
  const { status, stdout, stderr } = spawnSync(git, args, { cwd, env, encoding: "utf8" });
  return { status, stdout, stderr };
}

const root = mkdtempSync(join(tmpdir(), "git-guard-"));
let repos = 0;
// A repo with one commit, one stash and a staged change.
function repo() {
  const dir = join(root, String(repos++));
  mkdirSync(dir);
  const git = (...args: string[]) => assert.equal(run(realGit, dir, args).status, 0, args.join(" "));
  git("init", "-q", "-b", "main");
  writeFileSync(join(dir, "a"), "1\n");
  git("add", "a");
  git("commit", "-qm", "init, no stash yet");
  writeFileSync(join(dir, "a"), "2\n");
  git("stash", "-q");
  writeFileSync(join(dir, "a"), "3\n");
  git("add", "a");
  return dir;
}

const dir = repo();
const stashRef = () => run(realGit, dir, ["rev-parse", "refs/stash"]).stdout;
const before = stashRef();
for (const args of [["stash"], ["stash", "push"], ["stash", "pop"], ["-C", dir, "stash", "list"], ["-c", "x.y=z", "stash"]]) {
  const { status, stderr } = run(guard, dir, args);
  assert.notEqual(status, 0, `git ${args.join(" ")} is refused`);
  assert.match(stderr, /refs\/stash/);
  assert.match(stderr, /commit/i);
  assert.match(stderr, /git diff HEAD > \/tmp\/wip\.patch/);
}
assert.equal(stashRef(), before, "refs/stash is untouched");

for (const args of [["status"], ["log", "--grep", "stash"], ["commit", "-m", "stash"], ["-C", "<dir>", "rev-parse", "HEAD"]]) {
  const [guarded, real] = [guard, realGit].map((git) => {
    const cwd = repo();
    return run(git, cwd, args.map((arg) => (arg === "<dir>" ? cwd : arg)));
  });
  assert.equal(real.status, 0, `git ${args.join(" ")} succeeds`);
  assert.deepEqual(guarded, real, `git ${args.join(" ")} passes through`);
}

rmSync(root, { recursive: true });
console.log("git-guard.sh: ok");
