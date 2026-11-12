// Self-check for sweep.ts: `npm --prefix .sandcastle run check`.
import assert from "node:assert/strict";
import { dead } from "./sweep.ts";

const root = "/repo/.sandcastle/worktrees";
const facts = {
  host: "desk",
  alive: (pid: number) => pid === 7, // this run
  done: new Set([667, 712]), // #667 closed by PR #712
  mounted: new Set([`${root}/agent-issue-800`]),
};
const found = dead(
  {
    worktrees: [
      { path: `${root}/agent-issue-667`, branch: "agent/issue-667" },
      { path: `${root}/agent-issue-741`, branch: "agent/issue-741" },
      { path: `${root}/agent-triage-9`, branch: "agent/triage-9" },
      { path: `${root}/agent-issue-800`, branch: "agent/triage-800" },
    ],
    branches: ["agent/issue-667", "agent/issue-741", "agent/triage-9", "agent/issue-12", "agent/fix-712", "agent/triage-800"],
    claims: [
      { ref: "refs/claims/pr-712", owner: "5@desk" },
      { ref: "refs/claims/issue-741", owner: "5@desk" },
      { ref: "refs/claims/issue-667", owner: "7@desk" },
      { ref: "refs/claims/issue-667", owner: "5@laptop" },
    ],
  },
  facts,
);

assert.deepEqual(found.worktrees, [`${root}/agent-issue-667`, `${root}/agent-triage-9`],
  "a done issue's and a triage's go; an open issue's stays to be resumed, and a running container's stays");
assert.deepEqual(found.branches, ["agent/issue-667", "agent/triage-9"],
  "#12 is open, agent/fix-712 isn't the factory's, and a kept worktree's branch stays");
assert.deepEqual(found.claims, ["refs/claims/pr-712"],
  "only this machine's dead runs' claims on done work: an open issue's is taken over when it's picked again");
console.log("sweep.ts: ok");
