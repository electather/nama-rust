// AI factory: ready-for-agent issues → implement → review → PR → CI → squash-merge.
// Run from the repo root: `npm --prefix .sandcastle run factory`  (the script cd's back to the root)
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir, hostname } from "node:os";
import { setTimeout as sleep } from "node:timers/promises";
import * as sandcastle from "@ai-hero/sandcastle";
import { docker } from "@ai-hero/sandcastle/sandboxes/docker";

const MAX_ISSUES = Number(process.env.MAX_ISSUES ?? 60); // issues started per run; MAX_ISSUES=1 → one issue, then stop
const MAX_PARALLEL = 3; // issues built at once
// `touch .sandcastle/STOP`: running issues finish, nothing new starts, later runs exit at once. `rm` it to resume.
const STOP_FILE = ".sandcastle/STOP";
const MODEL = "claude-opus-5-5";
const READY = "ready-for-agent";
const HUMAN = "ready-for-human";
const WORKING = "agent-working"; // on while the factory has the issue; off when it merges, is handed back or crashes
const BASE = "main";

type Issue = { number: number; title: string; parent?: number };
type Box = Awaited<ReturnType<typeof sandcastle.createSandbox>>;
class Blocked extends Error {}

const sh = (cmd: string, ...args: string[]) =>
  execFileSync(cmd, args, { encoding: "utf8" }).trim();
const gh = (...args: string[]) => sh("gh", ...args);
const git = (...args: string[]) => sh("git", ...args);
const ok = (cmd: string, ...args: string[]) =>
  spawnSync(cmd, args, { stdio: "ignore" }).status === 0;

const cacheRoot = `${homedir()}/.cache/nama-sandcastle`;
const cargoCache = `${cacheRoot}/cargo-registry`;
const pnpmStore = `${cacheRoot}/pnpm-store`;
for (const dir of [cargoCache, pnpmStore]) mkdirSync(dir, { recursive: true });

// One factory per machine: they'd share the caches above and race on the same local branches.
const PID_FILE = `${cacheRoot}/factory.pid`;
const alive = (pid: number) => { try { process.kill(pid, 0); return true; } catch { return false; } };
try {
  writeFileSync(PID_FILE, String(process.pid), { flag: "wx" });
} catch {
  const pid = Number(readFileSync(PID_FILE, "utf8"));
  if (alive(pid)) {
    console.error(`Another factory is already running on this machine (pid ${pid}).`);
    process.exit(1);
  }
  writeFileSync(PID_FILE, String(process.pid)); // stale: its run was killed
}
process.on("exit", () => rmSync(PID_FILE, { force: true }));
const sandbox = () =>
  docker({
    mounts: [
      // ponytail: shared registry only; per-worktree target/ rebuilds from scratch. Add sccache if builds drag.
      { hostPath: cargoCache, sandboxPath: "~/.cargo/registry" },
      { hostPath: pnpmStore, sandboxPath: "~/.local/share/pnpm/store" },
      // your mattpocock skills (tdd, code-review, …) available to agents in the container
      { hostPath: "~/.agents/skills", sandboxPath: "~/.claude/skills", readonly: true },
      // ponytail is a plugin, not in ~/.agents/skills; same path in and out so implement-prompt.md finds it either way
      { hostPath: "~/.claude/plugins/cache/ponytail/ponytail", sandboxPath: "~/.claude/plugins/cache/ponytail/ponytail", readonly: true },
    ],
  });

// Run one agent pass; throws Blocked if the agent gave up.
async function agent(box: Box, issue: Issue, step: string, extra: Record<string, string> = {}) {
  const result = await box.run({
    name: `${step} #${issue.number}`,
    agent: sandcastle.claudeCode(MODEL, { effort: "high" }),
    promptFile: `.sandcastle/${step}-prompt.md`,
    promptArgs: {
      ISSUE_NUMBER: String(issue.number),
      ISSUE_TITLE: issue.title,
      BRANCH: branchOf(issue),
      PARENT: issue.parent ? `#${issue.parent}` : "none",
      ...extra,
    },
    maxIterations: 3,
    idleTimeoutSeconds: 1800, // cold cargo builds are silent for a long time
  });
  const reason = [...result.stdout.matchAll(/<blocked>(.*?)<\/blocked>/g)]
    .map((m) => m[1])
    .find((r) => r !== "one-line reason"); // ignore the prompt's own example
  if (reason) throw new Blocked(`${step}: ${reason}`);
  return result;
}

const branchOf = (i: Issue) => `agent/issue-${i.number}`;
const ahead = (branch: string) =>
  Number(git("rev-list", "--count", `origin/${BASE}..${branch}`)) > 0;

type ListedIssue = {
  number: number;
  title: string;
  parent: { number: number } | null;
  subIssuesSummary: { total: number };
  blockedBy: { nodes: { state: string }[] };
};

// Unblocked = no open native blocked-by (see docs/agents/issue-tracker.md).
// An issue with sub-issues is never built itself: its children are, and closeFinishedParents closes it.
function readyIssues(): Issue[] {
  const issues: ListedIssue[] = JSON.parse(
    gh("issue", "list", "--label", READY, "--state", "open", "--limit", "100",
      // blockedBy, not issue_dependencies_summary: the summary lags seconds behind closes/edits
      "--json", "number,title,parent,subIssuesSummary,blockedBy"),
  );
  return issues
    .filter((i) => i.subIssuesSummary.total === 0)
    .filter((i) => !i.blockedBy.nodes.some((b) => b.state === "OPEN"))
    .map((i) => ({ number: i.number, title: i.title, parent: i.parent?.number }))
    .sort((a, b) => a.number - b.number);
}

// GitHub doesn't close a parent when its last sub-issue closes, and issues it blocks would wait forever.
// A sweep rather than a per-merge check: it also catches children closed by hand or a summary that lagged.
function closeFinishedParents() {
  const open: { number: number; subIssuesSummary: { total: number; completed: number } }[] = JSON.parse(
    gh("issue", "list", "--label", READY, "--state", "open", "--limit", "100", "--json", "number,subIssuesSummary"),
  );
  for (const { number, subIssuesSummary: s } of open) {
    if (s.total === 0 || s.completed < s.total) continue;
    gh("issue", "comment", String(number), "--body", `🏭 All ${s.total} sub-issues are closed, so this one is done.`);
    gh("issue", "close", String(number));
    console.log(`  ✓ parent #${number} closed (${s.total}/${s.total} sub-issues done)`);
  }
}

// `Closes #N` closes the issue a few seconds after the merge, not during it.
async function waitUntilClosed(issue: Issue) {
  for (let tries = 0; tries < 12; tries++) {
    if (gh("issue", "view", String(issue.number), "--json", "state", "--jq", ".state") === "CLOSED") return;
    await sleep(5_000);
  }
}

function handBack(issue: Issue, why: string) {
  console.error(`  ✗ #${issue.number} → ${HUMAN}: ${why}`);
  gh("issue", "edit", String(issue.number), "--remove-label", READY, "--add-label", HUMAN);
  gh(
    "issue", "comment", String(issue.number), "--body",
    `🏭 The factory stopped on this one and handed it back.\n\n**Why:** ${why}\n\nWork so far is on branch \`${branchOf(issue)}\` (local and, if it got that far, pushed with a PR).`,
  );
  // Siblings blocked by this one stall silently; the parent is where that shows.
  if (issue.parent) {
    gh("issue", "comment", String(issue.parent), "--body", `🏭 #${issue.number} was handed back to a human: ${why}`);
  }
}

// The reviewer writes the PR body with the `pr` skill; fall back to a stub if it didn't.
function prBody(issue: Issue, reviewOut: string) {
  const written = [...reviewOut.matchAll(/<pr-body>([\s\S]*?)<\/pr-body>/g)].pop()?.[1]?.trim();
  const links = issue.parent ? `Closes #${issue.number}\nPart of #${issue.parent}` : `Closes #${issue.number}`;
  return `${links}\n\n${written || "Built by the sandcastle factory (implement → code-review → fix)."}`;
}

// Runs the functions passed to it one at a time, in call order.
function queue() {
  let tail: Promise<unknown> = Promise.resolve();
  return <T>(fn: () => Promise<T>): Promise<T> => {
    const next = tail.then(fn);
    tail = next.catch(() => {});
    return next;
  };
}
const serially = queue(); // merges
// Concurrent `git worktree add`s race on .git/worktrees (one reads another's half-written commondir).
const oneSandboxAtATime = queue();

async function build(issue: Issue): Promise<{ box: Box; body: string }> {
  const box = await oneSandboxAtATime(() =>
    sandcastle.createSandbox({
      branch: branchOf(issue),
      baseBranch: `origin/${BASE}`,
      sandbox: sandbox(),
    }),
  );
  try {
    await agent(box, issue, "implement");
    if (!ahead(branchOf(issue))) throw new Blocked("implementer made no commits");
    const review = await agent(box, issue, "review");
    return { box, body: prBody(issue, review.stdout) };
  } catch (e) {
    await box.close();
    throw e;
  }
}

// Waits for the whole CI workflow run on the branch's pushed commit. `gh pr checks --watch` isn't
// enough: a job behind `needs` (test after lint) has no check until it starts, so it can pass early.
// "stale": main moved on (another issue merged), so this run no longer says whether the PR can merge.
async function waitForChecks(branch: string): Promise<"pass" | "fail" | "stale"> {
  const sha = git("rev-parse", branch);
  const stale = () => {
    ok("git", "fetch", "origin"); // a failed fetch (another worker's fetch holds the lock) just checks the old ref
    return !ok("git", "merge-base", "--is-ancestor", `origin/${BASE}`, branch);
  };
  let run = "";
  for (let i = 0; i < 30 && !run; i++) {
    await sleep(10_000);
    if (stale()) return "stale"; // a conflicting PR gets no run at all
    run = gh("run", "list", "--workflow", "CI", "--commit", sha, "--event", "pull_request",
      "--json", "databaseId", "--jq", ".[0].databaseId // empty");
  }
  if (!run) throw new Blocked(`no CI run started for ${sha.slice(0, 7)} within 5 minutes`);
  // Async, not spawnSync: the other issue's agents keep running while this one waits.
  const watch = spawn("gh", ["run", "watch", run, "--exit-status", "--interval", "30"], { stdio: "ignore" });
  let wasStale = false;
  const poll = setInterval(() => {
    if (stale()) { wasStale = true; watch.kill(); }
  }, 60_000);
  const code = await new Promise<number | null>((resolve) => watch.on("close", resolve));
  clearInterval(poll);
  if (wasStale) {
    ok("gh", "run", "cancel", run); // frees a runner for the rebased push
    return "stale";
  }
  return code === 0 ? "pass" : "fail";
}

// Rebases onto main if another issue merged first, then pushes.
// (A PR that conflicts with main gets no pull_request CI run at all.)
async function syncAndPush(issue: Issue, box: Box) {
  const branch = branchOf(issue);
  git("fetch", "origin");
  if (!ok("git", "merge-base", "--is-ancestor", `origin/${BASE}`, branch)) {
    await agent(box, issue, "fix", { PROBLEM: `It is behind origin/${BASE} (other issues merged first) and may conflict.` });
  }
  git("push", "--force-with-lease", "origin", `${branch}:${branch}`);
}

// One fix attempt if CI fails. If main moves while CI runs, resync and wait for CI again.
async function greenCi(issue: Issue, box: Box, pr: string) {
  const branch = branchOf(issue);
  let fixed = false;
  for (;;) {
    const result = await waitForChecks(branch);
    if (result === "pass") return;
    if (result === "stale") {
      await syncAndPush(issue, box);
      continue;
    }
    if (fixed) throw new Blocked(`CI still failing on PR #${pr} after one fix attempt`);
    await agent(box, issue, "fix", { PROBLEM: `CI is failing on PR #${pr}.` });
    git("push", "--force-with-lease", "origin", `${branch}:${branch}`);
    fixed = true;
  }
}

// Runs as soon as review is done, in parallel with the other running issues.
async function openPr(issue: Issue, box: Box, body: string): Promise<string> {
  const branch = branchOf(issue);
  await syncAndPush(issue, box);
  const existing = gh("pr", "list", "--head", branch, "--state", "open", "--json", "number", "--jq", ".[0].number");
  if (existing) gh("pr", "edit", existing, "--body", body); // resumed run: refresh the body for the new review
  const pr =
    existing ||
    gh("pr", "create", "--base", BASE, "--head", branch, "--title", issue.title, "--body", body)
      .split("/").pop()!;
  await greenCi(issue, box, pr);
  return pr;
}

// Runs one at a time (see `serially`), so each branch is checked against the latest main.
async function mergePr(issue: Issue, box: Box, pr: string) {
  const branch = branchOf(issue);
  git("fetch", "origin");
  if (!ok("git", "merge-base", "--is-ancestor", `origin/${BASE}`, branch)) {
    await syncAndPush(issue, box); // another issue merged since CI went green: CI again on the rebased branch
    await greenCi(issue, box, pr);
  }
  gh("pr", "merge", pr, "--squash");
  ok("git", "push", "origin", "--delete", branch);
  console.log(`  ✓ #${issue.number} merged via PR #${pr}`);
  if (issue.parent) {
    await waitUntilClosed(issue);
    closeFinishedParents();
  }
}

// build → PR → green CI in parallel; only the merge waits its turn.
async function pipeline(issue: Issue) {
  const { box, body } = await build(issue);
  let merged = false;
  try {
    const pr = await openPr(issue, box, body);
    await serially(() => mergePr(issue, box, pr));
    merged = true;
  } finally {
    await box.close();
    if (merged) ok("git", "branch", "-D", branchOf(issue)); // squash-merged; the local branch is dead weight
  }
}

// Factories on other machines: creating a ref on origin is atomic, so only one of them gets an issue.
// The claim is a fresh commit each time: pushing a commit the ref already points at would "succeed" as up to date.
const claimRef = (i: Issue) => `refs/claims/issue-${i.number}`;
function claim(issue: Issue): boolean {
  const commit = git("commit-tree", `origin/${BASE}^{tree}`, "-p", `origin/${BASE}`,
    "-m", `claim #${issue.number} by ${process.pid}@${hostname()} ${Date.now()}`);
  return ok("git", "push", `--force-with-lease=${claimRef(issue)}:`, "origin", `${commit}:${claimRef(issue)}`);
}
function release(issue: Issue) {
  ok("gh", "issue", "edit", String(issue.number), "--remove-label", WORKING);
  ok("git", "push", "origin", "--delete", claimRef(issue));
}

// Never shrinks: a merged issue stays open for a few seconds (`Closes #N` lags), so it mustn't be picked again.
const taken = new Set<number>();
const inFlight = new Set<Issue>();
let crashed = false;

// One of MAX_PARALLEL slots: as soon as its issue is merged or handed back, it picks the next one.
async function worker() {
  while (!crashed && taken.size < MAX_ISSUES && !existsSync(STOP_FILE)) {
    // No await between picking and `taken.add`, so two slots can't pick the same issue.
    git("fetch", "origin");
    closeFinishedParents();
    const issue = readyIssues().find((i) => !taken.has(i.number));
    if (!issue) {
      if (inFlight.size === 0) return; // nothing in flight can unblock anything: done
      await sleep(60_000); // an in-flight merge may unblock an issue
      continue;
    }
    taken.add(issue.number);
    if (!claim(issue)) {
      console.log(`  #${issue.number} is being built by another factory; skipping`);
      continue;
    }
    inFlight.add(issue);
    console.log(`\n=== #${issue.number} ${issue.title} (${inFlight.size}/${MAX_PARALLEL} running) ===`);
    try {
      gh("issue", "edit", String(issue.number), "--add-label", WORKING);
      await pipeline(issue);
    } catch (e) {
      if (e instanceof Blocked) handBack(issue, e.message);
      else {
        // Infra failure (docker, auth, Claude limit): don't relabel; let in-flight issues finish, pick nothing new.
        console.error(`  ✗ #${issue.number} crashed:`, e);
        crashed = true;
      }
    } finally {
      release(issue);
      inFlight.delete(issue);
    }
  }
}

// Ctrl-C: let go of claimed issues so the next run (here or elsewhere) can resume them.
process.on("SIGINT", () => {
  for (const issue of inFlight) release(issue);
  process.exit(130);
});

await Promise.all(Array.from({ length: MAX_PARALLEL }, worker));
console.log(crashed ? "Stopped after a crash." : existsSync(STOP_FILE) ? `${STOP_FILE} found: stopped.` : "Queue empty. Done.");
process.exit(crashed ? 1 : 0);
