// AI factory: ready-for-agent issues → implement → review → PR → CI → squash-merge.
// Run from the repo root: `npm --prefix .sandcastle run factory`  (the script cd's back to the root)
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { setTimeout as sleep } from "node:timers/promises";
import * as sandcastle from "@ai-hero/sandcastle";
import { docker } from "@ai-hero/sandcastle/sandboxes/docker";

const MAX_CYCLES = Number(process.env.MAX_CYCLES ?? 20); // MAX_CYCLES=1 → one batch, then stop
const MAX_PARALLEL = 1; // one issue at a time
const MODEL = "claude-opus-5-5";
const READY = "ready-for-agent";
const HUMAN = "ready-for-human";
const BASE = "main";

type Issue = { number: number; title: string };
type Box = Awaited<ReturnType<typeof sandcastle.createSandbox>>;
class Blocked extends Error {}

const sh = (cmd: string, ...args: string[]) =>
  execFileSync(cmd, args, { encoding: "utf8" }).trim();
const gh = (...args: string[]) => sh("gh", ...args);
const git = (...args: string[]) => sh("git", ...args);
const ok = (cmd: string, ...args: string[]) =>
  spawnSync(cmd, args, { stdio: "ignore" }).status === 0;

const cargoCache = `${homedir()}/.cache/nama-sandcastle/cargo-registry`;
mkdirSync(cargoCache, { recursive: true });
const sandbox = () =>
  docker({
    mounts: [
      // ponytail: shared registry only; per-worktree target/ rebuilds from scratch. Add sccache if builds drag.
      { hostPath: cargoCache, sandboxPath: "~/.cargo/registry" },
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

// Unblocked = no open native blocked-by (see docs/agents/issue-tracker.md).
function readyIssues(): Issue[] {
  const issues: Issue[] = JSON.parse(
    gh("issue", "list", "--label", READY, "--state", "open", "--limit", "100", "--json", "number,title"),
  );
  return issues
    .filter(
      (i) =>
        // blockedBy, not issue_dependencies_summary: the summary lags seconds behind closes/edits
        gh("issue", "view", String(i.number), "--json", "blockedBy", "--jq", '[.blockedBy.nodes[] | select(.state == "OPEN")] | length') === "0",
    )
    .sort((a, b) => a.number - b.number);
}

function handBack(issue: Issue, why: string) {
  console.error(`  ✗ #${issue.number} → ${HUMAN}: ${why}`);
  gh("issue", "edit", String(issue.number), "--remove-label", READY, "--add-label", HUMAN);
  gh(
    "issue", "comment", String(issue.number), "--body",
    `🏭 The factory stopped on this one and handed it back.\n\n**Why:** ${why}\n\nWork so far is on branch \`${branchOf(issue)}\` (local and, if it got that far, pushed with a PR).`,
  );
}

// The reviewer writes the PR body with the `pr` skill; fall back to a stub if it didn't.
function prBody(issue: Issue, reviewOut: string) {
  const written = [...reviewOut.matchAll(/<pr-body>([\s\S]*?)<\/pr-body>/g)].pop()?.[1]?.trim();
  return `Closes #${issue.number}\n\n${written || "Built by the sandcastle factory (implement → code-review → fix)."}`;
}

async function build(issue: Issue): Promise<{ box: Box; body: string }> {
  const box = await sandcastle.createSandbox({
    branch: branchOf(issue),
    baseBranch: `origin/${BASE}`,
    sandbox: sandbox(),
  });
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

// "pass" | "fail" | "none" (repo has no CI yet; the reviewer's cargo run is the gate)
async function waitForChecks(pr: string): Promise<"pass" | "fail" | "none"> {
  await sleep(20_000); // ponytail: let GitHub register the checks; poll for check-suite creation if 20s proves flaky
  const r = spawnSync("gh", ["pr", "checks", pr, "--watch", "--fail-fast", "--interval", "30"], { encoding: "utf8" });
  if (r.status === 0) return "pass";
  if (/no checks reported/i.test(r.stderr + r.stdout)) return "none";
  return "fail";
}

async function ship(issue: Issue, box: Box, body: string) {
  const branch = branchOf(issue);
  git("fetch", "origin");
  if (!ok("git", "merge-base", "--is-ancestor", `origin/${BASE}`, branch)) {
    await agent(box, issue, "fix", { PROBLEM: `It is behind origin/${BASE} (other issues merged first) and may conflict.` });
  }
  git("push", "--force-with-lease", "origin", `${branch}:${branch}`);

  const existing = gh("pr", "list", "--head", branch, "--state", "open", "--json", "number", "--jq", ".[0].number");
  if (existing) gh("pr", "edit", existing, "--body", body); // resumed run: refresh the body for the new review
  const pr =
    existing ||
    gh("pr", "create", "--base", BASE, "--head", branch, "--title", issue.title, "--body", body)
      .split("/").pop()!;

  let checks = await waitForChecks(pr);
  if (checks === "fail") {
    await agent(box, issue, "fix", { PROBLEM: `CI is failing on PR #${pr}.` });
    git("push", "--force-with-lease", "origin", `${branch}:${branch}`);
    checks = await waitForChecks(pr);
  }
  if (checks === "fail") throw new Blocked(`CI still failing on PR #${pr} after one fix attempt`);

  gh("pr", "merge", pr, "--squash");
  ok("git", "push", "origin", "--delete", branch);
  console.log(`  ✓ #${issue.number} merged via PR #${pr} (CI: ${checks})`);
}

let crashed = false;
for (let cycle = 1; cycle <= MAX_CYCLES && !crashed; cycle++) {
  git("fetch", "origin");
  // ponytail: fixed batches of MAX_PARALLEL instead of a worker pool; leftovers go in the next cycle
  const batch = readyIssues().slice(0, MAX_PARALLEL);
  if (batch.length === 0) {
    console.log("Queue empty. Done.");
    break;
  }
  console.log(`\n=== Cycle ${cycle}: ${batch.map((i) => `#${i.number}`).join(", ")} ===`);

  const built = await Promise.allSettled(batch.map(build));

  // Merges go one at a time so each branch is checked against the latest main.
  for (const [i, outcome] of built.entries()) {
    const issue = batch[i]!;
    if (outcome.status === "rejected") {
      if (outcome.reason instanceof Blocked) handBack(issue, outcome.reason.message);
      else {
        // Infra failure (docker, auth, network): don't relabel; stop after this cycle.
        console.error(`  ✗ #${issue.number} crashed:`, outcome.reason);
        crashed = true;
      }
      continue;
    }
    const { box, body } = outcome.value;
    let merged = false;
    try {
      await ship(issue, box, body);
      merged = true;
    } catch (e) {
      if (e instanceof Blocked) handBack(issue, e.message);
      else {
        console.error(`  ✗ #${issue.number} crashed while shipping:`, e);
        crashed = true;
      }
    } finally {
      await box.close();
      if (merged) ok("git", "branch", "-D", branchOf(issue)); // squash-merged; the local branch is dead weight
    }
  }
}

process.exit(crashed ? 1 : 0);
