// What a run that was killed outright leaves behind: its worktrees (each with a build of tens of GB), local
// branches, stopped containers, `agent-working` labels and claims on origin. A run that ends on its own cleans up
// as it goes; a killed one never reaches that code, so the next run on the machine sweeps before it starts. Only
// one factory runs per machine (main.ts, PID_FILE), so none of it is in use then. An open issue's worktree and
// branch stay, since the next run resumes them; a triage's never do, since triage commits nothing.
//
// `npx --prefix .sandcastle tsx .sandcastle/sweep.ts` (from the repo root) lists what the next run would remove;
// add `--apply` to remove it now. With a run going, its worktrees, containers and claims count as in use and stay.
import { execFileSync, spawnSync } from "node:child_process";
import { hostname } from "node:os";

/** What the factory may have left on this machine and on origin. */
export type Leftovers = {
  worktrees: { path: string; branch: string }[];
  branches: string[];
  claims: { ref: string; owner: string }[];
};

/** What says whether a leftover is dead. */
export type Facts = {
  host: string;
  alive: (pid: number) => boolean;
  /** Closed issues and merged or closed PRs, which share GitHub's numbers. */
  done: Set<number>;
  /** Worktrees a running container works in. */
  mounted: Set<string>;
};

/** The factory's own branches: a build's or a triage's. Other `agent/*` branches are people's or sessions'. */
const OURS = /^agent\/(issue|triage)-(\d+)$/;
const finished = (branch: string, done: Set<number>) => {
  const [, kind, number] = OURS.exec(branch) ?? [];
  return kind === "triage" || (kind === "issue" && done.has(Number(number)));
};

/** Which leftovers are dead: their issue or PR is done, and no running run holds them. */
export function dead(left: Leftovers, facts: Facts) {
  const worktrees = left.worktrees.filter((w) => !facts.mounted.has(w.path) && finished(w.branch, facts.done));
  const kept = new Set(left.worktrees.filter((w) => !worktrees.includes(w)).map((w) => w.branch));
  return {
    worktrees: worktrees.map((w) => w.path),
    branches: left.branches.filter((b) => !kept.has(b) && finished(b, facts.done)),
    claims: left.claims
      .filter(({ ref, owner }) => {
        const [pid, host] = owner.split("@");
        return host === facts.host && !facts.alive(Number(pid)) && facts.done.has(Number(/-(\d+)$/.exec(ref)?.[1]));
      })
      .map((c) => c.ref),
  };
}

const sh = (cmd: string, ...args: string[]) => execFileSync(cmd, args, { encoding: "utf8" }).trim();
const ok = (cmd: string, ...args: string[]) => spawnSync(cmd, args, { stdio: "ignore" }).status === 0;
const lines = (text: string) => text.split("\n").filter(Boolean);
const alive = (pid: number) => { try { process.kill(pid, 0); return true; } catch { return false; } };
const WORKING = "agent-working";
// The Dockerfile labels the image, so a rebuild's old, now untagged one can be told from other projects'.
const IMAGE_LABEL = "nama-sandcastle";

function leftovers(root: string): Leftovers {
  const worktrees = sh("git", "worktree", "list", "--porcelain").split("\n\n").map((block) => ({
    path: /^worktree (.*)$/m.exec(block)?.[1] ?? "",
    branch: /^branch refs\/heads\/(.*)$/m.exec(block)?.[1] ?? "",
  }));
  const claims = lines(sh("git", "ls-remote", "origin", "refs/claims/*")).map((l) => l.split("\t"));
  // The claim commit's message names its owner: "claim … by pid@host …".
  if (claims.length) sh("git", "fetch", "--no-write-fetch-head", "origin", ...claims.map(([, ref]) => ref));
  const factorys = (path: string) => path.startsWith(`${root}/.sandcastle/worktrees/`);
  // A session may have checked out one of the factory's branches to look at it; git won't delete that one.
  const elsewhere = new Set(worktrees.filter((w) => !factorys(w.path)).map((w) => w.branch));
  return {
    worktrees: worktrees.filter((w) => factorys(w.path) && OURS.test(w.branch)),
    branches: lines(sh("git", "branch", "--list", "agent/*", "--format=%(refname:short)"))
      .filter((b) => OURS.test(b) && !elsewhere.has(b)),
    claims: claims.map(([sha, ref]) => ({
      ref,
      owner: / by (\S+)/.exec(sh("git", "log", "-1", "--format=%s", sha))?.[1] ?? "",
    })),
  };
}

/** Which of `numbers` are closed issues or merged or closed PRs, in one GraphQL call. */
function done(numbers: number[]): Set<number> {
  if (!numbers.length) return new Set();
  const fields = numbers
    .map((n) => `n${n}: issueOrPullRequest(number: ${n}) { ... on Issue { state } ... on PullRequest { state } }`)
    .join(" ");
  const query = `query($owner: String!, $name: String!) { repository(owner: $owner, name: $name) { ${fields} } }`;
  const repo = JSON.parse(sh("gh", "api", "graphql", "-F", "owner={owner}", "-F", "name={repo}", "-f", `query=${query}`))
    .data.repository as Record<string, { state?: string } | null>;
  return new Set(numbers.filter((n) => ["CLOSED", "MERGED"].includes(repo[`n${n}`]?.state ?? "")));
}

/** The worktrees running sandcastle containers work in. */
function mounted(): Set<string> {
  const running = lines(sh("docker", "ps", "-q", "--filter", "name=sandcastle-"));
  if (!running.length) return new Set();
  const workspace = '{{range .Mounts}}{{if eq .Destination "/home/agent/workspace"}}{{.Source}}{{end}}{{end}}';
  return new Set(lines(sh("docker", "inspect", "-f", workspace, ...running)));
}

/**
 * Finds what killed runs left in the repo at `root` and on origin, and removes it unless `dryRun`. Returns a line
 * per kind of leftover found, empty when there was none.
 */
export function sweepLeftovers(root: string, { dryRun = false } = {}): string[] {
  const left = leftovers(root);
  const numbers = [...left.worktrees.map((w) => w.branch), ...left.branches, ...left.claims.map((c) => c.ref)]
    .map((name) => Number(/-(\d+)$/.exec(name)?.[1]))
    .filter(Number.isInteger);
  const found = dead(left, { host: hostname(), alive, done: done([...new Set(numbers)]), mounted: mounted() });
  const containers = lines(sh("docker", "ps", "-aq", "--filter", "name=sandcastle-", "--filter", "status=exited"));
  const images = lines(sh("docker", "images", "-q", "--filter", "dangling=true", "--filter", `label=${IMAGE_LABEL}`));
  const labelled = lines(sh("gh", "issue", "list", "--state", "closed", "--label", WORKING, "--json", "number",
    "--jq", ".[].number"));
  const report: [string, string[]][] = [
    ["worktrees", found.worktrees.map((p) => p.split("/").pop() ?? p)],
    // With its tip, so a branch whose commits never reached a PR can be made again (`git branch <name> <tip>`)
    // until git collects them, two weeks on.
    ["branches", found.branches.map((b) => `${b}@${sh("git", "rev-parse", "--short", b)}`)],
    ["claims", found.claims.map((r) => r.replace("refs/claims/", ""))],
    ["containers", containers],
    ["images", images],
    [`${WORKING} on closed issues`, labelled.map((n) => `#${n}`)],
  ];
  if (!dryRun) {
    const failed: string[] = [];
    const run = (what: string, ...cmd: [string, ...string[]]) => ok(...cmd) || failed.push(what);
    for (const path of found.worktrees) {
      ok("git", "worktree", "unlock", path); // main.ts locks each so an agent's `git worktree prune` skips it
      run(path, "git", "worktree", "remove", "--force", path);
    }
    for (const branch of found.branches) run(branch, "git", "branch", "-D", branch);
    for (const ref of found.claims) run(ref, "git", "push", "origin", "--delete", ref);
    if (containers.length) run("containers", "docker", "rm", ...containers);
    if (images.length) run("images", "docker", "rmi", ...images);
    for (const n of labelled) run(`#${n}'s label`, "gh", "issue", "edit", n, "--remove-label", WORKING);
    if (failed.length) report.push(["couldn't remove", failed]);
  }
  return report.filter(([, items]) => items.length).map(([kind, items]) => `${kind}: ${items.join(", ")}`);
}

if (import.meta.main) {
  const apply = process.argv.includes("--apply");
  const found = sweepLeftovers(process.cwd(), { dryRun: !apply });
  console.log(found.length ? `${apply ? "Removed" : "Would remove"}:\n  ${found.join("\n  ")}` : "Nothing left behind.");
}
