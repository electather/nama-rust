// AI factory: ready-for-agent issues → implement → review → PR → CI → squash-merge.
// Run from the repo root: `npm --prefix .sandcastle run factory`  (the script cd's back to the root)
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir, hostname } from "node:os";
import { setTimeout as sleep } from "node:timers/promises";
import { inspect } from "node:util";
import * as sandcastle from "@ai-hero/sandcastle";
import { docker } from "@ai-hero/sandcastle/sandboxes/docker";
import { recorder, type Outcome, type Stage, type Waiting } from "./status.ts";

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
// The issue was closed while being built: drop it, don't count it as a crash.
class Cancelled extends Error {}

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

// launchd.log gets one timestamped line per thing that happens; status.md (see status.ts) the state of every issue.
const LOGS = ".sandcastle/logs";
mkdirSync(LOGS, { recursive: true });
const RUN = `${process.pid}@${hostname()}`;
const REPO_URL = gh("repo", "view", "--json", "url", "--jq", ".url");
const record = recorder(LOGS, RUN, REPO_URL);
const log = (msg: string) => console.log(`${new Date().toTimeString().slice(0, 8)} ${msg}`);

// The gist of an error in one line: execFileSync puts git's reason in stderr, sandcastle on the line after a colon.
function oneLine(e: unknown): string {
  const err = e as { message?: string; stderr?: unknown };
  const lines = String(err?.message ?? e).split("\n").map((l) => l.trim()).filter(Boolean);
  const head = lines[0]?.endsWith(":") && lines[1] ? `${lines[0]} ${lines[1]}` : (lines[0] ?? "unknown error");
  const why = String(err?.stderr ?? "").split("\n").map((l) => l.trim()).find((l) => /rejected|error|fatal/i.test(l));
  return why ? `${head} — ${why}` : head;
}

// Each issue's current stage, for "review → pr (review took 4m)" and "crashed at pr".
const stageOf = new Map<number, { stage: Stage; at: number }>();
const took = (since: number) => `${Math.round((Date.now() - since) / 60_000)}m`;
function stage(issue: Issue, next: Stage) {
  if (next !== "claimed") stopIfClosed(issue); // a cheap check at every boundary, besides the watcher
  const prev = stageOf.get(issue.number);
  stageOf.set(issue.number, { stage: next, at: Date.now() });
  if (prev) log(`#${issue.number} ${prev.stage} → ${next} (${prev.stage} took ${took(prev.at)})`);
  record({ kind: "stage", issue: issue.number, title: issue.title, stage: next });
}
// One per in-flight issue. Aborted with a Cancelled once the issue is closed.
const aborts = new Map<number, AbortController>();
const isClosed = (issue: Issue) =>
  spawnSync("gh", ["issue", "view", String(issue.number), "--json", "state", "--jq", ".state"], { encoding: "utf8" })
    .stdout?.trim() === "CLOSED"; // gh failing reads as open: only a definite "closed" stops work
function stopIfClosed(issue: Issue) {
  const abort = aborts.get(issue.number);
  if (abort && !abort.signal.aborted && isClosed(issue)) abort.abort(new Cancelled("the issue was closed"));
  abort?.signal.throwIfAborted();
}

const note = (issue: Issue, text: string) => {
  log(`#${issue.number} ${text}`);
  record({ kind: "note", issue: issue.number, text });
};
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
    signal: aborts.get(issue.number)?.signal, // aborting kills the agent and rejects with the reason (a Cancelled)
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
  subIssuesSummary: { total: number; completed: number };
  blockedBy: { nodes: { number: number; state: string }[] };
};

// Unblocked = no open native blocked-by (see docs/agents/issue-tracker.md).
// An issue with sub-issues is never built itself: its children are, and closeFinishedParents closes it.
function readyIssues(): { ready: Issue[]; waiting: Waiting[] } {
  const issues: ListedIssue[] = JSON.parse(
    gh("issue", "list", "--label", READY, "--state", "open", "--limit", "100",
      // blockedBy, not issue_dependencies_summary: the summary lags seconds behind closes/edits
      "--json", "number,title,parent,subIssuesSummary,blockedBy"),
  );
  const ready: Issue[] = [];
  const waiting: Waiting[] = [];
  for (const i of issues.sort((a, b) => a.number - b.number)) {
    const s = i.subIssuesSummary;
    const blockers = i.blockedBy.nodes.filter((b) => b.state === "OPEN").map((b) => `#${b.number}`);
    if (s.total > 0) waiting.push({ issue: i.number, why: `parent: waits for ${s.total - s.completed} of ${s.total} sub-issues` });
    else if (blockers.length) waiting.push({ issue: i.number, why: `blocked by ${blockers.join(", ")} (open)` });
    else ready.push({ number: i.number, title: i.title, parent: i.parent?.number });
  }
  return { ready, waiting };
}

// GitHub doesn't close a parent when its last sub-issue closes, and issues it blocks would wait forever.
// A sweep rather than a per-merge check: it also catches children closed by hand or a summary that lagged.
function closeFinishedParents() {
  const open: { number: number; subIssuesSummary: { total: number; completed: number } }[] = JSON.parse(
    gh("issue", "list", "--label", READY, "--state", "open", "--limit", "100", "--json", "number,subIssuesSummary"),
  );
  for (const { number, subIssuesSummary: s } of open) {
    if (s.total > 0 && s.completed === s.total) closeParent(number, s.total);
  }
}
function closeParent(number: number, total: number) {
  gh("issue", "comment", String(number), "--body", `🏭 All ${total} sub-issues are closed, so this one is done.`);
  gh("issue", "close", String(number));
  log(`✓ parent #${number} closed (${total}/${total} sub-issues done)`);
}

// The sweep only sees parents labelled ready-for-agent. A parent of something the factory merged is closed
// whatever its labels, and so on up: its own parent may have just finished too.
function closeIfFinished(number: number) {
  const p: { state: string; subIssuesSummary: { total: number; completed: number }; parent: { number: number } | null } =
    JSON.parse(gh("issue", "view", String(number), "--json", "state,subIssuesSummary,parent"));
  const s = p.subIssuesSummary;
  if (p.state !== "OPEN" || s.total === 0 || s.completed < s.total) return;
  closeParent(number, s.total);
  if (p.parent) closeIfFinished(p.parent.number);
}

// `Closes #N` closes the issue a few seconds after the merge, not during it.
async function waitUntilClosed(issue: Issue) {
  for (let tries = 0; tries < 12; tries++) {
    if (gh("issue", "view", String(issue.number), "--json", "state", "--jq", ".state") === "CLOSED") return;
    await sleep(5_000);
  }
}

function handBack(issue: Issue, why: string) {
  log(`⚠ #${issue.number} → ${HUMAN}: ${why}`);
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
    stage(issue, "implement");
    await agent(box, issue, "implement");
    if (!ahead(branchOf(issue))) throw new Blocked("implementer made no commits");
    stage(issue, "review");
    const review = await agent(box, issue, "review");
    return { box, body: prBody(issue, review.stdout) };
  } catch (e) {
    await box.close();
    throw e;
  }
}

// Waits for the whole CI workflow run on the branch's pushed commit. `gh pr checks --watch` isn't
// enough: a job behind `needs` (test after lint) has no check until it starts, so it can pass early.
// "stale": another issue merged and the PR now conflicts with main, so this run can't merge.
async function waitForChecks(issue: Issue): Promise<"pass" | "fail" | "stale"> {
  const branch = branchOf(issue);
  const sha = git("rev-parse", branch);
  const stale = () => {
    ok("git", "fetch", "origin"); // a failed fetch (another worker's fetch holds the lock) just checks the old ref
    return conflicts(branch);
  };
  let run = "";
  const signal = aborts.get(issue.number)?.signal;
  for (let i = 0; i < 30 && !run; i++) {
    await sleep(10_000);
    signal?.throwIfAborted();
    if (stale()) return "stale"; // a conflicting PR gets no run at all
    run = gh("run", "list", "--workflow", "CI", "--commit", sha, "--event", "pull_request",
      "--json", "databaseId", "--jq", ".[0].databaseId // empty");
  }
  if (!run) throw new Blocked(`no CI run started for ${sha.slice(0, 7)} within 5 minutes`);
  const url = `${REPO_URL}/actions/runs/${run}`;
  log(`#${issue.number} CI running on ${sha.slice(0, 7)}: ${url}`);
  record({ kind: "ci", issue: issue.number, url });
  // Async, not spawnSync: the other issue's agents keep running while this one waits.
  const watch = spawn("gh", ["run", "watch", run, "--exit-status", "--interval", "30"], { stdio: "ignore" });
  let wasStale = false;
  const poll = setInterval(() => {
    if (signal?.aborted) watch.kill();
    else if (stale()) { wasStale = true; watch.kill(); }
  }, 60_000);
  const code = await new Promise<number | null>((resolve) => watch.on("close", resolve));
  clearInterval(poll);
  signal?.throwIfAborted();
  if (wasStale) {
    ok("gh", "run", "cancel", run); // frees a runner for the rebased push
    return "stale";
  }
  return code === 0 ? "pass" : "fail";
}

// Only a conflict with main needs a rebase (and a fresh CI run): a PR that's merely behind still
// squash-merges cleanly, and its pull_request run already tested it merged into main.
// ponytail: two PRs that merge cleanly but break each other are caught by main's push run, not
// before the merge. Rebase whenever main moves if that ever lands a red main.
// merge-tree exits non-zero on a conflict (or any error, which then just rebases).
const conflicts = (branch: string) => !ok("git", "merge-tree", "--write-tree", `origin/${BASE}`, branch);

// Rebases onto main if another issue's merge conflicts with this one, then pushes.
// (A PR that conflicts with main gets no pull_request CI run at all.)
async function syncAndPush(issue: Issue, box: Box) {
  const branch = branchOf(issue);
  git("fetch", "origin");
  if (conflicts(branch)) {
    note(issue, "rebase");
    await agent(box, issue, "fix", { PROBLEM: `It is behind origin/${BASE} (other issues merged first) and may conflict.` });
  }
  git("push", "--force-with-lease", "origin", `${branch}:${branch}`);
}

// One fix attempt if CI fails. If main moves while CI runs, resync and wait for CI again.
async function greenCi(issue: Issue, box: Box, pr: string) {
  const branch = branchOf(issue);
  let fixed = false;
  for (;;) {
    const result = await waitForChecks(issue);
    log(`#${issue.number} CI ${result}`);
    if (result === "pass") return;
    if (result === "stale") {
      await syncAndPush(issue, box);
      continue;
    }
    if (fixed) throw new Blocked(`CI still failing on PR #${pr} after one fix attempt`);
    note(issue, "CI fix");
    await agent(box, issue, "fix", { PROBLEM: `CI is failing on PR #${pr}.` });
    git("push", "--force-with-lease", "origin", `${branch}:${branch}`);
    fixed = true;
  }
}

// Runs as soon as review is done, in parallel with the other running issues.
async function openPr(issue: Issue, box: Box, body: string): Promise<string> {
  const branch = branchOf(issue);
  stage(issue, "pr");
  await syncAndPush(issue, box);
  const existing = gh("pr", "list", "--head", branch, "--state", "open", "--json", "number", "--jq", ".[0].number");
  if (existing) gh("pr", "edit", existing, "--body", body); // resumed run: refresh the body for the new review
  const pr =
    existing ||
    gh("pr", "create", "--base", BASE, "--head", branch, "--title", issue.title, "--body", body)
      .split("/").pop()!;
  log(`#${issue.number} PR #${pr}: ${REPO_URL}/pull/${pr}`);
  record({ kind: "pr", issue: issue.number, pr });
  stage(issue, "ci");
  await greenCi(issue, box, pr);
  return pr;
}

// Runs one at a time (see `serially`), so each branch is checked against the latest main.
async function mergePr(issue: Issue, box: Box, pr: string) {
  const branch = branchOf(issue);
  stage(issue, "merging");
  git("fetch", "origin");
  if (conflicts(branch)) {
    await syncAndPush(issue, box); // another issue's merge conflicts with this one: CI again on the rebased branch
    await greenCi(issue, box, pr);
  }
  gh("pr", "merge", pr, "--squash");
  ok("git", "push", "origin", "--delete", branch);
  log(`✓ #${issue.number} merged via PR #${pr}`);
  if (issue.parent) {
    await waitUntilClosed(issue);
    try {
      closeFinishedParents();
      closeIfFinished(issue.parent);
    } catch (e) {
      log(`#${issue.number} merged, but closing its parent #${issue.parent} failed: ${oneLine(e)}`); // not retried: close it by hand
    }
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
function tryClaim(issue: Issue): boolean {
  const commit = git("commit-tree", `origin/${BASE}^{tree}`, "-p", `origin/${BASE}`,
    "-m", `claim #${issue.number} by ${process.pid}@${hostname()} ${Date.now()}`);
  return ok("git", "push", `--force-with-lease=${claimRef(issue)}:`, "origin", `${commit}:${claimRef(issue)}`);
}
// Returns undefined once the issue is ours, or who holds it.
// A claim from this machine whose pid is dead is stale (a killed run): only one factory runs here, so take it over.
function claim(issue: Issue): string | undefined {
  if (tryClaim(issue)) return;
  const owner = claimOwner(issue);
  const [pid, host] = owner.split("@");
  if (host !== hostname() || alive(Number(pid))) return owner;
  log(`#${issue.number} taking over the claim of ${owner}: that run is gone`);
  ok("git", "push", "origin", "--delete", claimRef(issue));
  return tryClaim(issue) ? undefined : claimOwner(issue); // another machine may win the gap
}
// Who holds a claim: the claim commit's message says "claim #N by pid@host …".
function claimOwner(issue: Issue): string {
  if (!ok("git", "fetch", "origin", claimRef(issue))) return "another factory";
  return git("log", "-1", "--format=%s", "FETCH_HEAD").match(/ by (\S+)/)?.[1] ?? "another factory";
}
function release(issue: Issue) {
  ok("gh", "issue", "edit", String(issue.number), "--remove-label", WORKING);
  ok("git", "push", "origin", "--delete", claimRef(issue));
}

// Never shrinks: a merged issue stays open for a few seconds (`Closes #N` lags), so it mustn't be picked again.
const taken = new Set<number>();
const inFlight = new Set<Issue>();
const claimedElsewhere = new Map<number, string>();
const outcomes: { issue: number; outcome: Outcome }[] = [];
let crashed = "";

function finish(issue: Issue, outcome: Outcome, why?: string) {
  outcomes.push({ issue: issue.number, outcome });
  record({ kind: "done", issue: issue.number, outcome, why });
}

// Logged and recorded only when it changes, not on every 60s poll.
let lastWaiting = "";
function noteWaiting(waiting: Waiting[]) {
  const all = [...waiting, ...[...claimedElsewhere].map(([issue, who]) => ({ issue, why: `claimed by ${who}` }))];
  const text = all.map((w) => `#${w.issue} ${w.why}`).join("; ");
  if (text === lastWaiting) return;
  lastWaiting = text;
  log(`waiting: ${text || "nothing"}`);
  record({ kind: "waiting", list: all });
}

// The next issue this factory now holds, "wait" if none is free yet, or "done".
// Synchronous throughout: no await between picking and `taken.add`, so two slots can't pick the same issue.
function pickNext(): Issue | "wait" | "done" {
  for (;;) {
    git("fetch", "origin");
    closeFinishedParents();
    const { ready, waiting } = readyIssues();
    noteWaiting(waiting);
    const issue = ready.find((i) => !taken.has(i.number));
    if (!issue) return inFlight.size === 0 ? "done" : "wait"; // nothing in flight can unblock anything: done
    taken.add(issue.number);
    const owner = claim(issue);
    if (!owner) return issue;
    claimedElsewhere.set(issue.number, owner);
    noteWaiting(waiting);
  }
}

// One of MAX_PARALLEL slots: as soon as its issue is merged or handed back, it picks the next one.
async function worker() {
  while (!crashed && taken.size < MAX_ISSUES && !existsSync(STOP_FILE)) {
    let issue: Issue | "wait" | "done";
    try {
      issue = pickNext();
    } catch (e) {
      // e.g. git or gh failing for everyone (network, auth, an Xcode licence prompt): a crash, not the end of
      // the run. In-flight issues finish and release their claims; nothing new starts.
      const why = `picking the next issue failed: ${oneLine(e)}`;
      appendFileSync(`${LOGS}/crash-pick.log`, `${new Date().toISOString()} ${RUN}\n${inspect(e)}\n\n`);
      log(`✗ ${why} (details: ${LOGS}/crash-pick.log)`);
      crashed ||= why;
      return;
    }
    if (issue === "done") return;
    if (issue === "wait") {
      await sleep(60_000); // an in-flight merge may unblock an issue
      continue;
    }
    inFlight.add(issue);
    log(`=== #${issue.number} ${issue.title} (${inFlight.size}/${MAX_PARALLEL} running) ===`);
    stage(issue, "claimed");
    const started = Date.now();
    aborts.set(issue.number, new AbortController());
    // Until the merge starts (the merge itself closes the issue), a closed issue stops its build.
    const abort = aborts.get(issue.number)!;
    const watcher = setInterval(() => {
      if (abort.signal.aborted || stageOf.get(issue.number)?.stage === "merging" || !isClosed(issue)) return;
      log(`#${issue.number} was closed: stopping it`);
      abort.abort(new Cancelled("the issue was closed"));
    }, 60_000);
    try {
      gh("issue", "edit", String(issue.number), "--add-label", WORKING);
      await pipeline(issue);
      finish(issue, "merged");
    } catch (e) {
      // The signal, not `instanceof`: sandcastle may hand the reason back wrapped in an Effect FiberFailure.
      if (abort.signal.aborted) {
        const why = `${(abort.signal.reason as Error).message} during ${stageOf.get(issue.number)?.stage ?? "start"}`;
        ok("gh", "pr", "close", branchOf(issue), "--comment", "🏭 The issue was closed, so the factory stopped building it.");
        log(`⊘ #${issue.number} cancelled: ${why}`);
        finish(issue, "cancelled", why);
      } else if (e instanceof Blocked) {
        handBack(issue, e.message);
        finish(issue, "handed back", e.message);
      } else {
        // Infra failure (docker, auth, Claude limit): don't relabel; let in-flight issues finish, pick nothing new.
        const at = stageOf.get(issue.number)?.stage ?? "start";
        const why = `crashed at ${at}: ${oneLine(e)}`;
        appendFileSync(`${LOGS}/crash-issue-${issue.number}.log`, `${new Date().toISOString()} ${RUN}\n${inspect(e)}\n\n`);
        log(`✗ #${issue.number} ${why} (details: ${LOGS}/crash-issue-${issue.number}.log)`);
        finish(issue, "crashed", why);
        crashed ||= `#${issue.number} ${why}`;
      }
    } finally {
      log(`#${issue.number} done after ${took(started)}`);
      release(issue);
      clearInterval(watcher);
      aborts.delete(issue.number);
      inFlight.delete(issue);
      stageOf.delete(issue.number);
    }
  }
}

function summary(reason: string) {
  log(`=== Run ended: ${reason} ===`);
  for (const o of ["merged", "handed back", "cancelled", "crashed", "interrupted"] as const) {
    const list = outcomes.filter((x) => x.outcome === o).map((x) => `#${x.issue}`);
    if (list.length) log(`  ${o}: ${list.join(", ")}`);
  }
  try {
    const { ready, waiting } = readyIssues();
    log(`  still ready: ${ready.map((i) => `#${i.number}`).join(", ") || "none"}`);
    for (const w of waiting) log(`  waiting: #${w.issue} ${w.why}`);
  } catch (e) {
    log(`  couldn't list the queue: ${oneLine(e)}`);
  }
  record({ kind: "run-end", reason });
}

// Ctrl-C: let go of claimed issues so the next run (here or elsewhere) can resume them.
process.on("SIGINT", () => {
  for (const issue of inFlight) {
    release(issue);
    finish(issue, "interrupted", `Ctrl-C at ${stageOf.get(issue.number)?.stage ?? "start"}`);
  }
  log("=== Run ended: interrupted (Ctrl-C) ===");
  record({ kind: "run-end", reason: "interrupted (Ctrl-C)" });
  process.exit(130);
});

log(`=== Factory ${RUN} started: ${MAX_PARALLEL} slots, at most ${MAX_ISSUES} issues ===`);
record({ kind: "run-start", slots: MAX_PARALLEL });
await Promise.all(Array.from({ length: MAX_PARALLEL }, worker));
summary(
  crashed ? `${crashed}; finished in-flight work, started nothing new`
  : existsSync(STOP_FILE) ? `${STOP_FILE} found`
  : taken.size >= MAX_ISSUES ? `MAX_ISSUES (${MAX_ISSUES}) reached`
  : "queue empty",
);
process.exit(crashed ? 1 : 0);
