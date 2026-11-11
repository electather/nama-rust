// AI factory, three loops in one process:
// - triage: needs-triage issues, one at a time → an agent closes, specifies (ready-for-agent), splits, or asks
//   (needs-decision).
// - implement: ready-for-agent issues with no open PR → implement → review → push → PR, then lets go.
// - babysit: qualifying PRs (see babysittable) → CI (rebase / one fix if needed) → up to main's tip → squash-merge.
// Run from the repo root: `npm --prefix .sandcastle run factory`  (the script cd's back to the root)
import { execFile, execFileSync, spawn, spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir, hostname } from "node:os";
import { setTimeout as sleep } from "node:timers/promises";
import { inspect, parseEnv, promisify } from "node:util";
import * as sandcastle from "@ai-hero/sandcastle";
import { docker } from "@ai-hero/sandcastle/sandboxes/docker";
import { podman } from "@ai-hero/sandcastle/sandboxes/podman";
import { mergeStep } from "./merge.ts";
import { slotPool } from "./slots.ts";
import { recorder, type Outcome, type Stage, type Waiting } from "./status.ts";

const MAX_ISSUES = Number(process.env.MAX_ISSUES ?? 60); // issues started per run; MAX_ISSUES=1 → one issue, then stop
const MAX_PARALLEL = Number(process.env.MAX_PARALLEL ?? 2); // issues built at once
// `touch .sandcastle/STOP`: running issues finish, nothing new starts, later runs exit at once. `rm` it to resume.
const STOP_FILE = ".sandcastle/STOP";
// Opus for implement and review; fix (merge main, mend CI) is narrow enough for Sonnet, with Opus as fallback.
const OPUS = sandcastle.claudeCode("claude-opus-5-5", { effort: "high" });
const SONNET = sandcastle.claudeCode("claude-sonnet-5-5", { effort: "medium" });
const READY = "ready-for-agent";
const HUMAN = "ready-for-human";
const WORKING = "agent-working"; // on while a loop has the issue; off when that loop lets go of it
const TRIAGE = "needs-triage";
const DECISION = "needs-decision"; // triage's answer when only the maintainer can decide
// TRIAGE=off: leave needs-triage issues to a person. ONLY_ISSUE (build just that one) doesn't triage either.
const TRIAGING = process.env.TRIAGE !== "off" && !process.env.ONLY_ISSUE;
const BASE = "main";

type Issue = { number: number; title: string; parent?: number; branch: string };
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
const sccacheDir = `${cacheRoot}/sccache`;
for (const dir of [cargoCache, pnpmStore, sccacheDir]) mkdirSync(dir, { recursive: true });
// TokenEconomizer learns from every agent session (setup: README). TE=off or no clone skips it.
const teClone = process.env.TE_CLONE ?? `${homedir()}/Code/TokenEconomizer`;
// TokenEconomizer serializes its state with flock. Containers in a VM (macOS, Docker Desktop, OrbStack) can't see
// the host's flocks over the file share, so there the factory keeps its own state; on native Linux the host's
// sessions and the sandboxes share one kernel and one lock, so they learn together.
const inVm = process.platform !== "linux" ||
  /Docker Desktop|OrbStack/i.test(spawnSync("docker", ["info", "--format", "{{.OperatingSystem}}"], { encoding: "utf8" }).stdout ?? "");
const teState = inVm ? `${cacheRoot}/tokeneconomizer` : `${homedir()}/.tokeneconomizer`;
const teMounts = process.env.TE !== "off" && existsSync(`${teClone}/plugin/scripts/te-hook`)
  ? [
      { hostPath: teClone, sandboxPath: "~/tokeneconomizer", readonly: true }, // whole clone: plugin/lib symlinks into it
      { hostPath: teState, sandboxPath: "~/.tokeneconomizer" },
      // claude-settings.json holds the plugin's hooks.json with the mounted path; ${CLAUDE_PLUGIN_ROOT} isn't set here.
      { hostPath: `${process.cwd()}/.sandcastle/claude-settings.json`, sandboxPath: "~/.claude/settings.json", readonly: true },
    ]
  : [];
if (teMounts.length) mkdirSync(teState, { recursive: true });

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

// The sandboxes share OrbStack's one VM, which runs out of memory when they peak together. Every 30s a sandbox
// over MEM_LOG_GIB (default 4) logs its biggest processes to mem.log, so the command behind a spike has a name.
const MEM_LOG_GIB = Number(process.env.MEM_LOG_GIB ?? 4);
const exec = promisify(execFile);
const gib = (usage: string) => {
  const [, n, unit] = usage.match(/([\d.]+)\s*(\w+)/) ?? [];
  return Number(n) / ({ B: 2 ** 30, KiB: 2 ** 20, MiB: 2 ** 10, GiB: 1, TiB: 2 ** -10 }[unit] ?? 1);
};
setInterval(async () => {
  try {
    const { stdout } = await exec("docker", ["stats", "--no-stream", "--format", "{{.ID}}\t{{.Name}}\t{{.MemUsage}}"]);
    for (const [id, name, usage] of stdout.trim().split("\n").map((l) => l.split("\t"))) {
      if (!name?.startsWith("sandcastle-") || gib(usage) < MEM_LOG_GIB) continue;
      const worktree = (await exec("docker", ["inspect", "-f",
        '{{range .Mounts}}{{if eq .Destination "/home/agent/workspace"}}{{.Source}}{{end}}{{end}}', id])).stdout.trim();
      // Cut inside the container: one linker's argv alone runs to megabytes.
      const top = (await exec("docker", ["exec", id, "sh", "-c", "ps -eo rss=,etime=,args= --sort=-rss | head -8 | cut -c1-240"]))
        .stdout.trimEnd().split("\n").map((l) => `  ${l.trim()}`).join("\n");
      appendFileSync(`${LOGS}/mem.log`,
        `${new Date().toISOString()} ${worktree.split("/").pop() || name} ${usage}  (RSS KiB, elapsed, command)\n${top}\n`);
    }
  } catch {} // docker busy, or the sandbox closed mid-sample: skip this one
}, 30_000);

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
// A PR merged by hand counts as done, whatever stage the factory was at. `gh pr view <branch>` finds it
// after the merge too.
const prMerged = (issue: Issue) =>
  spawnSync("gh", ["pr", "view", branchOf(issue), "--json", "state", "--jq", ".state"], { encoding: "utf8" })
    .stdout?.trim() === "MERGED";
function stopIfClosed(issue: Issue) {
  const abort = aborts.get(issue.number);
  if (abort && !abort.signal.aborted && isClosed(issue)) abort.abort(new Cancelled("the issue was closed"));
  abort?.signal.throwIfAborted();
}

const note = (issue: Issue, text: string) => {
  log(`#${issue.number} ${text}`);
  record({ kind: "note", issue: issue.number, text });
};
// Rootless Podman behind a `docker` shim maps the host user to container root, so the docker provider's agent
// can't write its worktree; the podman provider maps it with --userns=keep-id instead.
const onPodman = /podman/i.test(spawnSync("docker", ["--version"], { encoding: "utf8" }).stdout ?? "");
const sandbox = () =>
  (onPodman ? podman : docker)({
    mounts: [
      // Each worktree's target/ starts empty; sccache (see Dockerfile) reuses dependency builds across sandboxes.
      { hostPath: cargoCache, sandboxPath: "~/.cargo/registry" },
      { hostPath: sccacheDir, sandboxPath: "~/.cache/sccache" },
      { hostPath: pnpmStore, sandboxPath: "~/.local/share/pnpm/store" },
      // The other skills are committed under .claude/skills, so the worktree (cut from origin/main) brings them.
      // ponytail is still a host skill; drop this mount once it's installed at project level.
      { hostPath: "~/.agents/skills/ponytail", sandboxPath: "~/.claude/skills/ponytail", readonly: true },
      ...teMounts,
    ],
  });

// Run one agent pass; throws Blocked if the agent gave up.
async function agent(box: Box, issue: Issue, step: string, extra: Record<string, string> = {}, model = OPUS) {
  const result = await box.run({
    name: `${step} #${issue.number}`,
    agent: model,
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

const slots = slotPool(MAX_PARALLEL);
// Triage has its own pool so it never holds up a build or a fix; it compiles little, so more fit at once.
const TRIAGE_PARALLEL = Number(process.env.TRIAGE_PARALLEL ?? 10);
const triageSlots = slotPool(TRIAGE_PARALLEL);
// Every agent running, for the usage slow-down, which counts them all.
const agentsBusy = () => slots.busy() + triageSlots.busy();

// Makes the branch mergeable: Sonnet first, then Opus from the last commit if Sonnet gives up.
async function fix(box: Box, issue: Issue, problem: string, what: string) {
  slotWaits.set(issue.number, { issue: issue.number, queue: "babysit", why: `${what} waits for a slot` });
  await slots.acquire(true).finally(() => slotWaits.delete(issue.number));
  try {
    aborts.get(issue.number)?.signal.throwIfAborted(); // closed while it waited for a slot
    return await agent(box, issue, "fix", { PROBLEM: problem }, SONNET);
  } catch (e) {
    if (!(e instanceof Blocked)) throw e;
    note(issue, `fix on Opus (${e.message})`);
    // Sonnet may have left a half-done merge behind.
    await box.exec("git merge --abort; git reset --hard HEAD");
    return await agent(box, issue, "fix", { PROBLEM: problem });
  } finally {
    slots.release();
  }
}

// Two fetches at once (a slot picking while another checks for staleness) fail with "cannot lock ref";
// that clash ended a whole run once, so retry before letting the error through.
function fetchOrigin() {
  for (let tries = 1; !ok("git", "fetch", "origin"); tries++) {
    if (tries === 3) return git("fetch", "origin"); // throws with git's reason
    spawnSync("sleep", ["5"]); // synchronous: pickNext must not await
  }
}

const branchOf = (i: Issue) => i.branch;
const ahead = (branch: string) =>
  Number(git("rev-list", "--count", `origin/${BASE}..${branch}`)) > 0;

type ListedIssue = {
  number: number;
  title: string;
  parent: { number: number } | null;
  subIssuesSummary: { total: number; completed: number };
  blockedBy: { nodes: { number: number; state: string }[] };
};

type IssueRef = { number: number; repository: { name: string; owner: { login: string } } };
type OpenPr = {
  number: number;
  isDraft: boolean;
  isCrossRepository: boolean;
  headRefName: string;
  author: { login: string };
  closingIssuesReferences: IssueRef[];
};
const [OWNER, REPO] = gh("repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner").split("/");
const ME = gh("api", "user", "--jq", ".login");
const inRepo = (i: IssueRef) => i.repository.owner.login === OWNER && i.repository.name === REPO;
const openPrs = (): OpenPr[] =>
  JSON.parse(gh("pr", "list", "--state", "open", "--limit", "100",
    "--json", "number,isDraft,isCrossRepository,headRefName,author,closingIssuesReferences"));
// Issue → the open PR that would close it. Any open PR, a draft or one by hand included: the implement loop leaves
// the issue alone, and the babysitter takes the PR if it qualifies.
function prsByIssue(prs: OpenPr[]): Map<number, number> {
  const byIssue = new Map<number, number>();
  for (const pr of prs) {
    const fromBranch = pr.headRefName.match(/^agent\/issue-(\d+)$/)?.[1]; // a body that lost its `Closes #N`
    if (fromBranch) byIssue.set(Number(fromBranch), pr.number);
    for (const i of pr.closingIssuesReferences.filter(inRepo)) byIssue.set(i.number, pr.number);
  }
  return byIssue;
}

// Unblocked = no open native blocked-by (see docs/agents/issue-tracker.md).
// An issue with sub-issues is never built itself: its children are, and closeFinishedParents closes it.
function readyIssues(prs = openPrs()): { ready: Issue[]; waiting: Waiting[] } {
  const issues: ListedIssue[] = JSON.parse(
    gh("issue", "list", "--label", READY, "--state", "open", "--limit", "100",
      // blockedBy, not issue_dependencies_summary: the summary lags seconds behind closes/edits
      "--json", "number,title,parent,subIssuesSummary,blockedBy"),
  );
  const withPr = prsByIssue(prs);
  const ready: Issue[] = [];
  const waiting: Waiting[] = [];
  for (const i of issues.sort((a, b) => a.number - b.number)) {
    if (process.env.ONLY_ISSUE && i.number !== Number(process.env.ONLY_ISSUE)) continue; // ONLY_ISSUE=N → build just #N
    const s = i.subIssuesSummary;
    const blockers = i.blockedBy.nodes.filter((b) => b.state === "OPEN").map((b) => `#${b.number}`);
    if (withPr.has(i.number)) continue; // the babysitter's, or a person's
    if (s.total > 0) waiting.push({ issue: i.number, queue: "build", why: `parent: waits for ${s.total - s.completed} of ${s.total} sub-issues` });
    else if (blockers.length) waiting.push({ issue: i.number, queue: "build", why: `blocked by ${blockers.join(", ")} (open)` });
    else ready.push({ number: i.number, title: i.title, parent: i.parent?.number, branch: `agent/issue-${i.number}` });
  }
  return { ready, waiting };
}

type Job = { pr: string; issue: Issue; box?: Box };
// PRs to see through to merge: open, not a draft, opened by ME from a branch of this repo (a fork's PR would run
// its code in a sandbox, then merge), closing an open issue ME wrote that carries READY.
function babysittable(prs = openPrs()): Job[] {
  const jobs: Job[] = [];
  for (const pr of prs.sort((a, b) => a.number - b.number)) {
    if (pr.isDraft || pr.isCrossRepository || pr.author.login !== ME) continue;
    for (const ref of pr.closingIssuesReferences.filter(inRepo)) {
      const i: { title: string; state: string; author: { login: string }; labels: { name: string }[]; parent: { number: number } | null } =
        JSON.parse(gh("issue", "view", String(ref.number), "--json", "title,state,author,labels,parent"));
      if (i.state !== "OPEN" || i.author.login !== ME || !i.labels.some((l) => l.name === READY)) continue;
      // One issue stands for the PR: the one reported on, and handed back if the PR can't be made green.
      jobs.push({ pr: String(pr.number), issue: { number: ref.number, title: i.title, parent: i.parent?.number, branch: pr.headRefName } });
      break;
    }
  }
  return jobs;
}

// Issues to triage, oldest first. Only ME's and the flaky-test reporter's: the agent acts with ME's token, so a
// stranger's issue body mustn't steer it, even though only collaborators and the reporter can set the label.
function triageable(): Issue[] {
  const issues: { number: number; title: string; author: { login: string } }[] = JSON.parse(
    gh("issue", "list", "--label", TRIAGE, "--state", "open", "--limit", "100", "--json", "number,title,author"),
  );
  return issues
    .filter((i) => i.author.login === ME || i.author.login === "app/github-actions")
    .sort((a, b) => a.number - b.number)
    .map((i) => ({ number: i.number, title: i.title, branch: `agent/triage-${i.number}` }));
}

// GitHub doesn't close a parent when its last sub-issue closes, and issues it blocks would wait forever.
// A sweep rather than a per-merge check: it also catches children closed by hand or a summary that lagged.
// Parents carry no ready-for-agent label (docs/agents/triage-labels.md), so they're found through their labelled
// children; closed children keep the label. ponytail: the newest 1000 labelled issues, so very old parents drop out.
function closeFinishedParents() {
  const children: { parent: { number: number; state: string } | null }[] = JSON.parse(
    gh("issue", "list", "--label", READY, "--state", "all", "--limit", "1000", "--json", "parent"),
  );
  const parents = new Set(children.flatMap(({ parent }) => (parent?.state === "OPEN" ? [parent.number] : [])));
  for (const number of parents) {
    const s: { total: number; completed: number } = JSON.parse(
      gh("issue", "view", String(number), "--json", "subIssuesSummary", "--jq", ".subIssuesSummary"),
    );
    if (s.total > 0 && s.completed === s.total) closeParent(number, s.total);
  }
}
function closeParent(number: number, total: number) {
  gh("issue", "comment", String(number), "--body", `🏭 All ${total} sub-issues are closed, so this one is done.`);
  gh("issue", "close", String(number));
  log(`✓ parent #${number} closed (${total}/${total} sub-issues done)`);
}

// The sweep only sees parents of labelled children. A parent of something the factory merged is closed
// whatever its labels, and so on up: its own parent may have just finished too.
// GitHub updates a parent's subIssuesSummary a few seconds after a child closes, so one short is re-read
// before giving up: #139 stayed open when its last child's close hadn't landed in the count yet.
async function closeIfFinished(number: number) {
  for (let tries = 0; ; tries++) {
    const p: { state: string; subIssuesSummary: { total: number; completed: number }; parent: { number: number } | null } =
      JSON.parse(gh("issue", "view", String(number), "--json", "state,subIssuesSummary,parent"));
    const s = p.subIssuesSummary;
    if (p.state !== "OPEN" || s.total === 0 || s.completed < s.total - 1) return;
    if (s.completed === s.total) {
      closeParent(number, s.total);
      if (p.parent) await closeIfFinished(p.parent.number);
      return;
    }
    if (tries === 6) {
      log(`parent #${number} left open: ${s.completed}/${s.total} sub-issues closed after 30s`);
      return;
    }
    await sleep(5_000);
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
const newBox = (branch: string) =>
  oneSandboxAtATime(() =>
    sandcastle.createSandbox({
      branch,
      baseBranch: `origin/${BASE}`,
      sandbox: sandbox(),
      // Each box.run's git setup uses these, not run's own: the 10s default ran out on `git rev-parse` while five
      // builds loaded the host, crashing the run.
      timeouts: { gitSetupMs: 120_000 },
    }),
  );

async function build(issue: Issue): Promise<{ box: Box; body: string }> {
  const box = await newBox(branchOf(issue));
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

// The implement loop's whole job: build, push, open the PR. From there the babysitter has it, so a crash
// later on never builds the issue again.
async function implement(issue: Issue): Promise<Outcome> {
  const { box, body } = await build(issue);
  try {
    const branch = branchOf(issue);
    stage(issue, "pr");
    fetchOrigin(); // the lease needs origin's current view of the branch
    git("push", "--force-with-lease", "origin", `${branch}:${branch}`);
    const existing = gh("pr", "list", "--head", branch, "--state", "open", "--json", "number", "--jq", ".[0].number");
    if (existing) gh("pr", "edit", existing, "--body", body); // resumed run: refresh the body for the new review
    const pr =
      existing ||
      gh("pr", "create", "--base", BASE, "--head", branch, "--title", issue.title, "--body", body)
        .split("/").pop()!;
    log(`#${issue.number} PR #${pr}: ${REPO_URL}/pull/${pr}`);
    record({ kind: "pr", issue: issue.number, pr });
  } finally {
    await box.close();
  }
  return "pr opened";
}

// The babysitter opens a sandbox only when a PR needs an agent (a rebase or a CI fix), at origin's tip of the
// branch: the local branch may be missing (the PR came from another machine or by hand) or behind.
async function boxFor(job: Job): Promise<Box> {
  if (!job.box) {
    const branch = branchOf(job.issue);
    const box = await newBox(branch);
    await box.exec(`git reset --hard origin/${branch}`);
    job.box = box;
  }
  return job.box;
}
const push = (job: Job) => git("push", "--force-with-lease", "origin", `${branchOf(job.issue)}:${branchOf(job.issue)}`);

// Waits for the whole CI workflow run on the PR's head. `gh pr checks --watch` isn't enough: a job behind
// `needs` (every job after `changes`) has no check until it starts, so it can pass early.
// "stale": another PR merged and this one now conflicts with main, so it can't merge.
async function waitForChecks(job: Job): Promise<"pass" | "fail" | "stale"> {
  const { issue } = job;
  const head = `origin/${branchOf(issue)}`;
  const sha = git("rev-parse", head);
  const stale = () => {
    ok("git", "fetch", "origin"); // a failed fetch (another worker's fetch holds the lock) just checks the old ref
    return conflicts(head);
  };
  let run = "";
  const signal = aborts.get(issue.number)?.signal;
  // 15 minutes: during GitHub incidents a PR's first run has started up to 13 minutes late.
  for (let i = 0; i < 90 && !run; i++) {
    if (i > 0) await sleep(10_000);
    signal?.throwIfAborted();
    if (stale()) return "stale"; // a conflicting PR gets no run at all
    run = gh("run", "list", "--workflow", "CI", "--commit", sha, "--event", "pull_request",
      "--json", "databaseId", "--jq", ".[0].databaseId // empty");
  }
  // A missing run is GitHub's fault, not the work's: crash (keeps ready-for-agent, a later run babysits the PR
  // again) rather than hand back.
  if (!run) throw new Error(`no CI run started for ${sha.slice(0, 7)} within 15 minutes`);
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

// A conflict needs an agent (rebase); a PR merely behind main is updated on GitHub (updateBranch). Either way it
// runs CI again before merging: see merge.ts for why (#561).
// merge-tree exits non-zero on a conflict (or any error, which then just rebases).
const conflicts = (ref: string) => !ok("git", "merge-tree", "--write-tree", `origin/${BASE}`, ref);
const behind = (ref: string) => !ok("git", "merge-base", "--is-ancestor", `origin/${BASE}`, ref);

// Another PR's merge conflicts with this one: merge main in, then push.
async function rebase(job: Job) {
  note(job.issue, "rebase");
  await fix(await boxFor(job), job.issue, `It is behind origin/${BASE} (other issues merged first) and conflicts.`, "rebase");
  push(job);
}

// Main moved but doesn't conflict: GitHub merges it into the PR's branch, which starts a new CI run. No agent, no
// slot. A sandbox already open for this PR is moved to the new head, or a later rebase would push over it.
async function updateBranch(job: Job) {
  note(job.issue, "update to main");
  gh("pr", "update-branch", job.pr);
  fetchOrigin();
  await job.box?.exec(`git reset --hard origin/${branchOf(job.issue)}`);
}

// Whether main's latest finished push run failed (a superseded run is cancelled, so it doesn't count).
const mainRed = () =>
  gh("run", "list", "--branch", BASE, "--workflow", "CI", "--event", "push", "--limit", "10", "--json", "conclusion",
    "--jq", '[.[] | select(.conclusion != "" and .conclusion != "cancelled")][0].conclusion // empty') === "failure";

// A red main fails every PR's run. A fix agent would mend main in each PR, and those PRs would then conflict,
// so the PR holds no slot and waits until main's own fix has landed. Triage files that fix.
async function waitForGreenMain(job: Job) {
  log(`#${job.issue.number} CI failed while main is red: waiting for main`);
  note(job.issue, "waited for a red main");
  const signal = aborts.get(job.issue.number)?.signal;
  while (mainRed()) {
    await sleep(60_000);
    signal?.throwIfAborted();
  }
  fetchOrigin();
}

// One fix attempt if CI fails, unless main is red. If main moves while CI runs, rebase and wait for CI again.
async function greenCi(job: Job) {
  let fixed = false;
  for (;;) {
    const result = await waitForChecks(job);
    log(`#${job.issue.number} CI ${result}`);
    if (result === "pass") return;
    if (result === "stale") {
      await rebase(job);
      continue;
    }
    if (mainRed()) {
      await waitForGreenMain(job);
      const head = `origin/${branchOf(job.issue)}`;
      const step = mergeStep({ conflicts: conflicts(head), behind: behind(head) });
      if (step === "rebase") await rebase(job);
      if (step === "update") await updateBranch(job);
      if (step !== "merge") continue; // else main was red for another reason: this failure is the PR's own
    }
    if (fixed) throw new Blocked(`CI still failing on PR #${job.pr} after one fix attempt`);
    note(job.issue, "CI fix");
    await fix(await boxFor(job), job.issue, `CI is failing on PR #${job.pr}.`, "CI fix");
    push(job);
    fixed = true;
  }
}

// Runs one at a time (see `serially`), so no other factory merge lands between the last green run and the merge.
// The cost is that merges land at least one CI run apart, as in a merge queue. Main is fetched again after each
// run, since a hand merge may have moved it meanwhile.
async function mergePr(job: Job) {
  stage(job.issue, "merging");
  const head = `origin/${branchOf(job.issue)}`;
  for (;;) {
    fetchOrigin();
    const step = mergeStep({ conflicts: conflicts(head), behind: behind(head) });
    if (step === "merge") break;
    if (step === "rebase") await rebase(job);
    else await updateBranch(job);
    await greenCi(job);
  }
  // A push between the check and here (an agent, by hand) would merge an untested head; GitHub refuses that.
  gh("pr", "merge", job.pr, "--squash", "--match-head-commit", git("rev-parse", head));
  log(`✓ #${job.issue.number} merged via PR #${job.pr}`);
  await afterMerge(job.issue);
}

// The babysitter's whole job: green CI, then merge.
async function babysit(job: Job): Promise<Outcome> {
  record({ kind: "pr", issue: job.issue.number, pr: job.pr });
  try {
    stage(job.issue, "ci");
    await greenCi(job);
    await serially(() => mergePr(job));
  } finally {
    await job.box?.close();
  }
  ok("git", "branch", "-D", branchOf(job.issue)); // squash-merged; the local branch, if any, is dead weight
  return "merged";
}

// The triage loop's whole job: one agent pass that relabels, closes or splits the issue itself. The sandbox is a
// clean checkout of main to read and reproduce in; its branch never gets a commit.
async function triage(issue: Issue): Promise<Outcome> {
  const box = await newBox(issue.branch);
  let out: string;
  try {
    stage(issue, "triage");
    out = (await agent(box, issue, "triage", { ME })).stdout;
  } finally {
    await box.close();
    ok("git", "branch", "-D", issue.branch);
  }
  const decision = [...out.matchAll(/<triage>(.*?)<\/triage>/g)]
    .map((m) => m[1].trim())
    .filter((d) => !d.startsWith("ready-for-agent: test hard-codes /media")) // the prompt's own example
    .pop();
  const after: { state: string; labels: { name: string }[] } =
    JSON.parse(gh("issue", "view", String(issue.number), "--json", "state,labels"));
  // Left as it was, it would be picked again on every run.
  if (after.state === "OPEN" && after.labels.some((l) => l.name === TRIAGE)) {
    throw new Blocked(`triage ended with ${TRIAGE} still on it${decision ? ` (${decision})` : ""}`);
  }
  note(issue, decision ?? "triaged");
  return "triaged";
}

// Triage gave up: the maintainer decides instead.
function undecided(issue: Issue, why: string) {
  log(`⚠ #${issue.number} → ${DECISION}: ${why}`);
  gh("issue", "edit", String(issue.number), "--remove-label", TRIAGE, "--add-label", DECISION);
  gh("issue", "comment", String(issue.number), "--body",
    `🏭 The factory couldn't triage this one, so it waits for your decision.\n\n**Why:** ${why}`);
}

// After the PR merged, by the factory or by hand. The local branch is deleted by the caller once the
// sandbox's worktree is gone.
async function afterMerge(issue: Issue) {
  ok("git", "push", "origin", "--delete", branchOf(issue));
  if (!issue.parent) return;
  await waitUntilClosed(issue);
  try {
    closeFinishedParents();
    await closeIfFinished(issue.parent);
  } catch (e) {
    log(`#${issue.number} merged, but closing its parent #${issue.parent} failed: ${oneLine(e)}`); // not retried: close it by hand
  }
}

// Factories on other machines: creating a ref on origin is atomic, so only one of them gets an issue or a PR.
// The claim is a fresh commit each time: pushing a commit the ref already points at would "succeed" as up to date.
const issueClaim = (i: Issue) => `refs/claims/issue-${i.number}`;
const prClaim = (pr: string) => `refs/claims/pr-${pr}`;
const triageClaim = (i: Issue) => `refs/claims/triage-${i.number}`;
function tryClaim(ref: string, what: string): boolean {
  const commit = git("commit-tree", `origin/${BASE}^{tree}`, "-p", `origin/${BASE}`,
    "-m", `claim ${what} by ${process.pid}@${hostname()} ${Date.now()}`);
  return ok("git", "push", `--force-with-lease=${ref}:`, "origin", `${commit}:${ref}`);
}
// Returns undefined once the claim is ours, or who holds it.
// A claim from this machine whose pid is dead is stale (a killed run): only one factory runs here, so take it over.
function claim(ref: string, what: string): string | undefined {
  if (tryClaim(ref, what)) return;
  const owner = claimOwner(ref);
  const [pid, host] = owner.split("@");
  if (host !== hostname() || alive(Number(pid))) return owner;
  log(`${what} taking over the claim of ${owner}: that run is gone`);
  ok("git", "push", "origin", "--delete", ref);
  return tryClaim(ref, what) ? undefined : claimOwner(ref); // another machine may win the gap
}
// Who holds a claim: the claim commit's message says "claim … by pid@host …".
function claimOwner(ref: string): string {
  if (!ok("git", "fetch", "origin", ref)) return "another factory";
  return git("log", "-1", "--format=%s", "FETCH_HEAD").match(/ by (\S+)/)?.[1] ?? "another factory";
}
function release(issue: Issue, ref: string) {
  ok("gh", "issue", "edit", String(issue.number), "--remove-label", WORKING);
  ok("git", "push", "origin", "--delete", ref);
}

type Loop = "triage" | "implement" | "babysit";
// Never shrinks: a merged issue stays open for a few seconds (`Closes #N` lags), so it mustn't be picked again.
const taken = new Set<number>();
const watched = new Set<string>(); // PRs, likewise
const triaged = new Set<number>(); // issues the triage loop tried this run, whatever came of it
const triagePending = () => TRIAGING && triageable().some((i) => !triaged.has(i.number));
// By issue number: one issue is in at most one loop at a time.
const inFlight = new Map<number, { issue: Issue; loop: Loop; claim: string }>();
const claimedElsewhere = new Map<number, Waiting>(); // by issue
const slotWaits = new Map<number, Waiting>(); // in flight, but its fix waits for a slot
const outcomes: { issue: number; outcome: Outcome }[] = [];
let crashed = "";

function finish(issue: Issue, outcome: Outcome, why?: string) {
  outcomes.push({ issue: issue.number, outcome });
  record({ kind: "done", issue: issue.number, outcome, why });
}

// Logged and recorded only when it changes, not on every 60s poll.
let lastWaiting = "";
// Every loop's queue: what each would start next and why it hasn't yet.
function noteWaiting(prs: OpenPr[], { ready, waiting } = readyIssues(prs)) {
  const held = (pool: typeof slots, size: number, what: string) =>
    crashed ? "next run: this one crashed"
    : existsSync(STOP_FILE) ? "next run: STOP is set"
    : paused() ? "paused: Claude usage near its limit"
    : pool.busy() >= size ? `waits for a ${what} (${pool.busy()}/${size} busy)`
    : slowed() && agentsBusy() >= SLOW_PARALLEL ? `slowed: at most ${SLOW_PARALLEL} agents near the usage limit`
    : "starts next";
  const fresh = (n: number) => !taken.has(n) && !inFlight.has(n) && !claimedElsewhere.has(n);
  const all: Waiting[] = [
    ...ready.filter((i) => fresh(i.number)).map((i) => ({ issue: i.number, queue: "build" as const, why: held(slots, MAX_PARALLEL, "slot") })),
    ...waiting,
    ...(TRIAGING ? triageable() : []).filter((i) => !triaged.has(i.number) && fresh(i.number)).map((i) => ({
      issue: i.number,
      queue: "triage" as const,
      why: held(triageSlots, TRIAGE_PARALLEL, "triage slot"),
    })),
    ...slotWaits.values(),
    ...claimedElsewhere.values(),
  ];
  const text = all.map((w) => `${w.queue} #${w.issue} ${w.why}`).join("; ");
  if (text === lastWaiting) return;
  lastWaiting = text;
  log(`waiting: ${text || "nothing"}`);
  record({ kind: "waiting", list: all });
}

// The next issue this factory now holds, "wait" if none is free yet, or "done".
// Synchronous throughout: no await between picking and `taken.add`, so two slots can't pick the same issue.
function pickNext(): Issue | "wait" | "done" {
  for (;;) {
    fetchOrigin();
    closeFinishedParents();
    const prs = openPrs();
    const queued = readyIssues(prs);
    const { ready } = queued;
    // Not one still in the triage loop, which labelled it ready a moment ago: its release would strip the build's
    // agent-working label.
    const issue = ready.find((i) => !taken.has(i.number) && !inFlight.has(i.number));
    // Done only when nothing anywhere can still unblock an issue: no build, no PR left to merge, and nothing left
    // to triage (it may come out ready-for-agent).
    if (!issue) {
      noteWaiting(prs, queued);
      return inFlight.size === 0 && babysittable(prs).length === 0 && !triagePending() ? "done" : "wait";
    }
    taken.add(issue.number);
    const owner = claim(issueClaim(issue), `#${issue.number}`);
    if (owner) claimedElsewhere.set(issue.number, { issue: issue.number, queue: "build", why: `claimed by ${owner}` });
    noteWaiting(prs, queued);
    if (!owner) return issue;
  }
}

// Past USAGE_PAUSE of the Claude 5-hour limit, start nothing new until it resets: in-flight issues get the rest,
// fewer runs crash on the limit, and the subscription stays usable meanwhile. Past WEEKLY_STOP of the 7-day limit,
// likewise until the week resets: what's left of the week is the user's.
// The API has no usage endpoint for a setup-token, but every reply carries the unified rate-limit headers, so a
// 1-token Haiku call reads them. The OAuth token is only accepted with Claude Code's system prompt.
const USAGE_PAUSE = Number(process.env.USAGE_PAUSE ?? 0.95);
const WEEKLY_STOP = Number(process.env.WEEKLY_STOP ?? 0.92);
// Nearer the limits, fewer issues at once: past SLOW_5H of the 5-hour or SLOW_WEEKLY of the weekly limit, a new
// issue starts only while fewer than SLOW_PARALLEL run.
const SLOW_5H = Number(process.env.SLOW_5H ?? 0.8);
const SLOW_WEEKLY = Number(process.env.SLOW_WEEKLY ?? 0.85);
const SLOW_PARALLEL = Number(process.env.SLOW_PARALLEL ?? 1);
const OAUTH_TOKEN = parseEnv(readFileSync(".sandcastle/.env", "utf8")).CLAUDE_CODE_OAUTH_TOKEN;
type Usage = { used: number; resetsAt: number; weekly: number; weeklyResetsAt: number; readAt: number };
let usage: Usage | undefined;
async function readUsage(): Promise<Usage> {
  const res = await fetch("https://api.anthropic.com/v1/messages", {
    method: "POST",
    headers: {
      authorization: `Bearer ${OAUTH_TOKEN}`,
      "anthropic-beta": "oauth-2025-04-20",
      "anthropic-version": "2023-06-01",
      "content-type": "application/json",
    },
    body: JSON.stringify({
      model: "claude-haiku-5-5",
      max_tokens: 1,
      system: "You are Claude Code, Anthropic's official CLI for Claude.",
      messages: [{ role: "user", content: "hi" }],
    }),
  });
  // A 429 from the limit itself still carries the headers.
  const header = (name: string) => {
    const v = res.headers.get(`anthropic-ratelimit-unified-${name}`);
    if (v === null) throw new Error(`HTTP ${res.status}, no ${name} header`);
    return Number(v);
  };
  return {
    used: header("5h-utilization"),
    resetsAt: header("5h-reset") * 1000,
    weekly: header("7d-utilization"),
    weeklyResetsAt: header("7d-reset") * 1000,
    readAt: Date.now(),
  };
}
const overWeekly = (u: Usage) => u.weekly >= WEEKLY_STOP;
const pausedUntil = (u: Usage) => overWeekly(u) ? u.weeklyResetsAt : u.used >= USAGE_PAUSE ? u.resetsAt : 0;
const paused = () => !!usage && Date.now() < pausedUntil(usage);
const pct = (x: number) => `${Math.round(x * 100)}%`;
const when = (ms: number) => new Date(ms).toString().slice(0, 21); // the weekly reset can be days away
// Read at most once a minute, and not at all before a known reset. A failed read lets work start: the limit,
// if hit, still ends the run as an infra crash.
let usageRead: Promise<void> | undefined;
async function refreshUsage() {
  if (usage && (paused() || Date.now() - usage.readAt < 60_000)) return;
  usageRead ??= readUsage()
    .then((u) => {
      const was = paused();
      usage = u;
      if (paused() && !was) {
        log(overWeekly(u)
          ? `⏸ Claude weekly usage at ${pct(u.weekly)}: starting nothing new until ${when(u.weeklyResetsAt)}`
          : `⏸ Claude 5h usage at ${pct(u.used)}: starting nothing new until ${when(u.resetsAt)}`);
      } else if (!paused() && was) {
        log(`▶ Claude usage at ${pct(u.used)} (5h), ${pct(u.weekly)} (week): picking issues again`);
      }
    })
    .catch((e) => log(`couldn't read Claude usage, carrying on: ${oneLine(e)}`))
    .finally(() => { usageRead = undefined; });
  await usageRead;
}
const slowed = () => !!usage && (usage.used >= SLOW_5H || usage.weekly >= SLOW_WEEKLY);
// True while paused.
let wasSlowed = false;
async function overUsage(): Promise<boolean> {
  await refreshUsage();
  if (slowed() !== wasSlowed) {
    wasSlowed = slowed();
    log(wasSlowed
      ? `🐢 Claude usage at ${pct(usage!.used)} (5h), ${pct(usage!.weekly)} (week): at most ${SLOW_PARALLEL} slot(s) busy`
      : `🐇 Claude usage back under the slow-down: ${MAX_PARALLEL} slots`);
  }
  return paused();
}

// The implement loop: starts an issue whenever a slot is free (see slotPool). Started issues finish in the background.
async function dispatch() {
  const running = new Set<Promise<void>>();
  for (;;) {
    await slots.acquire();
    if (crashed || taken.size >= MAX_ISSUES || existsSync(STOP_FILE)) {
      slots.release();
      break;
    }
    let issue: Issue | "wait" | "done" = "wait";
    // busy() includes the slot just taken, hence `>`.
    if (!(await overUsage()) && !(slowed() && agentsBusy() > SLOW_PARALLEL)) {
      try {
        issue = pickNext();
      } catch (e) {
        // e.g. git or gh failing for everyone (network, auth, an Xcode licence prompt): a crash, not the end of
        // the run. In-flight issues finish and release their claims; nothing new starts.
        const why = `picking the next issue failed: ${oneLine(e)}`;
        appendFileSync(`${LOGS}/crash-pick.log`, `${new Date().toISOString()} ${RUN}\n${inspect(e)}\n\n`);
        log(`✗ ${why} (details: ${LOGS}/crash-pick.log)`);
        crashed ||= why;
      }
    }
    if (crashed || issue === "done") {
      slots.release();
      break;
    }
    if (issue === "wait") {
      slots.release();
      await sleep(60_000); // paused, slowed, or waiting for a merge to unblock an issue
      continue;
    }
    const i = issue;
    // The slot taken above is released when the issue leaves the loop (see track).
    const started: Promise<void> = track(i, "implement", issueClaim(i), () => implement(i))
      .finally(() => running.delete(started));
    running.add(started);
  }
  await Promise.all(running);
}

// The babysit loop: every minute, takes every PR that qualifies (see babysittable). Waiting on CI holds no slot;
// a rebase or CI fix takes one, ahead of new builds (see fix). Ends once the implement loop has ended and no PR is
// left, since until then a new PR may still come. A crash elsewhere doesn't stop it: PRs opened after one would
// otherwise sit until the next run, and a PR that crashes here isn't taken again this run (`watched`).
let implementDone = false;
async function babysitLoop() {
  const running = new Set<Promise<void>>();
  for (;;) {
    if (existsSync(STOP_FILE)) break;
    let jobs: Job[] = [];
    let prs: OpenPr[] = [];
    try {
      prs = openPrs();
      jobs = babysittable(prs);
    } catch (e) {
      const why = `listing PRs to babysit failed: ${oneLine(e)}`;
      appendFileSync(`${LOGS}/crash-pick.log`, `${new Date().toISOString()} ${RUN}\n${inspect(e)}\n\n`);
      log(`✗ ${why} (details: ${LOGS}/crash-pick.log)`);
      crashed ||= why;
      break;
    }
    let started = 0;
    for (const job of jobs) {
      // An issue still in the implement loop opened this PR moments ago: take it on the next pass.
      if (watched.has(job.pr) || inFlight.has(job.issue.number)) continue;
      const owner = claim(prClaim(job.pr), `PR #${job.pr}`);
      if (owner) {
        claimedElsewhere.set(job.issue.number, { issue: job.issue.number, queue: "babysit", why: `PR #${job.pr} babysat by ${owner}` });
        continue;
      }
      claimedElsewhere.delete(job.issue.number);
      watched.add(job.pr);
      started++;
      const p: Promise<void> = track(job.issue, "babysit", prClaim(job.pr), () => babysit(job))
        .finally(() => running.delete(p));
      running.add(p);
    }
    // The implement loop refreshes the queues only when it picks, and it can't while every slot is busy.
    try {
      noteWaiting(prs);
    } catch (e) {
      log(`couldn't refresh the waiting list: ${oneLine(e)}`); // status only: the next pass tries again
    }
    if (implementDone && running.size === 0 && started === 0) break;
    await sleep(60_000);
  }
  await Promise.all(running);
}

// The triage loop: up to TRIAGE_PARALLEL issues at once, oldest first, each in a triage slot. Ends once the
// implement loop has ended and nothing is left to triage; an issue parked meanwhile waits for the next run.
async function triageLoop() {
  if (!TRIAGING) return;
  const running = new Set<Promise<void>>();
  for (;;) {
    await triageSlots.acquire(); // released by track
    if (crashed || existsSync(STOP_FILE)) {
      triageSlots.release();
      break;
    }
    let issue: Issue | undefined;
    // agentsBusy() includes the slot just taken, hence `>`.
    if (!(await overUsage()) && !(slowed() && agentsBusy() > SLOW_PARALLEL)) {
      try {
        issue = triageable().find((i) => !triaged.has(i.number));
      } catch (e) {
        const why = `listing issues to triage failed: ${oneLine(e)}`;
        appendFileSync(`${LOGS}/crash-pick.log`, `${new Date().toISOString()} ${RUN}\n${inspect(e)}\n\n`);
        log(`✗ ${why} (details: ${LOGS}/crash-pick.log)`);
        crashed ||= why;
        triageSlots.release();
        break;
      }
    }
    if (!issue) {
      triageSlots.release();
      if (implementDone) break;
      await sleep(60_000); // paused, slowed, or nothing to triage until an agent parks something
      continue;
    }
    triaged.add(issue.number);
    const owner = claim(triageClaim(issue), `triage #${issue.number}`);
    if (owner) {
      log(`#${issue.number} triage skipped: claimed by ${owner}`);
      triageSlots.release();
      continue;
    }
    const i = issue;
    const p: Promise<void> = track(i, "triage", triageClaim(i), () => triage(i)).finally(() => running.delete(p));
    running.add(p);
  }
  await Promise.all(running);
}

// "true" unless gh says the label is gone: only a definite answer stops work.
const stillReady = (issue: Issue) =>
  spawnSync("gh", ["issue", "view", String(issue.number), "--json", "labels", "--jq", `[.labels[].name] | index("${READY}") != null`],
    { encoding: "utf8" }).stdout?.trim() !== "false";
const prClosed = (issue: Issue) =>
  spawnSync("gh", ["pr", "view", branchOf(issue), "--json", "state", "--jq", ".state"], { encoding: "utf8" })
    .stdout?.trim() === "CLOSED";

// One issue through one loop: implement ends at "pr opened", babysit at merged. Either may end handed back,
// cancelled or crashed instead.
async function track(issue: Issue, loop: Loop, claimRef: string, work: () => Promise<Outcome>) {
  inFlight.set(issue.number, { issue, loop, claim: claimRef });
  log(`=== ${loop} #${issue.number} ${issue.title} (${slots.busy()}/${MAX_PARALLEL} slots, ${triageSlots.busy()}/${TRIAGE_PARALLEL} triage slots busy, ${inFlight.size} in flight) ===`);
  stage(issue, "claimed");
  const started = Date.now();
  aborts.set(issue.number, new AbortController());
  // Until the merge starts (the merge itself closes the issue), a closed issue stops its work. A merged PR stops
  // it at any stage; the catch below then records it as merged. The babysitter also lets go of a PR that's
  // closed, or whose issue a person took back (no READY label).
  // Triage isn't watched: its agent closes and relabels the issue itself.
  const abort = aborts.get(issue.number)!;
  const watcher = setInterval(() => {
    if (abort.signal.aborted || loop === "triage") return;
    const merging = stageOf.get(issue.number)?.stage === "merging";
    const stop = (why: string) => {
      log(`#${issue.number} ${why}: stopping it`);
      abort.abort(new Cancelled(why));
    };
    if (prMerged(issue)) stop("the PR was merged");
    else if (!merging && isClosed(issue)) stop("the issue was closed");
    else if (loop === "babysit" && !merging && prClosed(issue)) stop("the PR was closed");
    else if (loop === "babysit" && !merging && !stillReady(issue)) stop(`the issue lost ${READY}`);
  }, 60_000);
  try {
    gh("issue", "edit", String(issue.number), "--add-label", WORKING);
    finish(issue, await work());
  } catch (e) {
    // Checked first: merging by hand makes whatever the factory was doing fail (a rejected push, a
    // cancelled CI run, `gh pr merge` on a merged PR), and none of that is a crash or a hand-back.
    if (prMerged(issue)) {
      log(`✓ #${issue.number} merged by hand`);
      await serially(() => afterMerge(issue));
      ok("git", "branch", "-D", branchOf(issue)); // the sandbox is closed by now
      finish(issue, "merged", "merged by hand");
    // The signal, not `instanceof`: sandcastle may hand the reason back wrapped in an Effect FiberFailure.
    } else if (abort.signal.aborted) {
      const why = `${(abort.signal.reason as Error).message} during ${stageOf.get(issue.number)?.stage ?? "start"}`;
      // The babysitter leaves the PR as it is: it may be a person's.
      if (loop === "implement") ok("gh", "pr", "close", branchOf(issue), "--comment", "🏭 The issue was closed, so the factory stopped building it.");
      log(`⊘ #${issue.number} cancelled: ${why}`);
      finish(issue, "cancelled", why);
    } else if (e instanceof Blocked) {
      if (loop === "triage") undecided(issue, e.message);
      else handBack(issue, e.message);
      finish(issue, "handed back", e.message);
    } else {
      // Infra failure (docker, auth, Claude limit): don't relabel; let in-flight issues finish, pick nothing new.
      // The next run picks the issue (no PR yet) or its PR (babysit) up again.
      const at = stageOf.get(issue.number)?.stage ?? "start";
      const why = `crashed at ${at}: ${oneLine(e)}`;
      appendFileSync(`${LOGS}/crash-issue-${issue.number}.log`, `${new Date().toISOString()} ${RUN}\n${inspect(e)}\n\n`);
      log(`✗ #${issue.number} ${why} (details: ${LOGS}/crash-issue-${issue.number}.log)`);
      finish(issue, "crashed", why);
      crashed ||= `#${issue.number} ${why}`;
    }
  } finally {
    log(`#${issue.number} ${loop} done after ${took(started)}`);
    release(issue, claimRef);
    clearInterval(watcher);
    aborts.delete(issue.number);
    inFlight.delete(issue.number);
    stageOf.delete(issue.number);
    if (loop === "triage") triageSlots.release();
    else if (loop === "implement") slots.release();
  }
}

function summary(reason: string) {
  log(`=== Run ended: ${reason} ===`);
  for (const o of ["merged", "pr opened", "triaged", "handed back", "cancelled", "crashed", "interrupted"] as const) {
    const list = outcomes.filter((x) => x.outcome === o).map((x) => `#${x.issue}`);
    if (list.length) log(`  ${o}: ${list.join(", ")}`);
  }
  try {
    const prs = openPrs();
    const { ready, waiting } = readyIssues(prs);
    log(`  still ready: ${ready.map((i) => `#${i.number}`).join(", ") || "none"}`);
    log(`  PRs to babysit: ${babysittable(prs).map((j) => `#${j.pr}`).join(", ") || "none"}`);
    for (const w of waiting) log(`  waiting: #${w.issue} ${w.why}`);
  } catch (e) {
    log(`  couldn't list the queue: ${oneLine(e)}`);
  }
  record({ kind: "run-end", reason });
}

// Ctrl-C: let go of claimed issues and PRs so the next run (here or elsewhere) can resume them.
process.on("SIGINT", () => {
  for (const { issue, claim } of inFlight.values()) {
    release(issue, claim);
    finish(issue, "interrupted", `Ctrl-C at ${stageOf.get(issue.number)?.stage ?? "start"}`);
  }
  log("=== Run ended: interrupted (Ctrl-C) ===");
  record({ kind: "run-end", reason: "interrupted (Ctrl-C)" });
  process.exit(130);
});

log(`=== Factory ${RUN} started: ${MAX_PARALLEL} slots, at most ${MAX_ISSUES} issues, babysitting ${ME}'s PRs ===`);
record({ kind: "run-start", slots: MAX_PARALLEL });
const babysitting = babysitLoop();
const triaging = triageLoop();
await dispatch();
implementDone = true;
await Promise.all([babysitting, triaging]);
summary(
  crashed ? `${crashed}; finished in-flight work, started nothing new`
  : existsSync(STOP_FILE) ? `${STOP_FILE} found`
  : taken.size >= MAX_ISSUES ? `MAX_ISSUES (${MAX_ISSUES}) reached`
  : "queue empty",
);
process.exit(crashed ? 1 : 0);
