// Self-check for status.ts: `npm --prefix .sandcastle run check`. Prints the rendered example.
import assert from "node:assert/strict";
import { type Event, render } from "./status.ts";

const repo = "https://github.com/o/r";
const at = (m: number) => new Date(Date.UTC(2026, 10, 7, 10, m)).toISOString();
const old = "1@a";
const run = "2@b";
const events: Event[] = [
  // A killed earlier run: #73 finished, #80 was left mid-build and must not show as in flight.
  { t: at(0), run: old, kind: "run-start", slots: 3 },
  { t: at(1), run: old, kind: "stage", issue: 73, title: "Watch | State", stage: "claimed" },
  { t: at(2), run: old, kind: "pr", issue: 73, pr: "116" },
  { t: at(3), run: old, kind: "note", issue: 73, text: "rebase" },
  { t: at(4), run: old, kind: "note", issue: 73, text: "rebase" },
  { t: at(9), run: old, kind: "done", issue: 73, outcome: "crashed", why: "crashed at ci: push rejected" },
  { t: at(9), run: old, kind: "stage", issue: 80, title: "Orphan", stage: "review" },
  // #90's implement loop opened its PR; its babysitting hasn't started.
  { t: at(5), run: old, kind: "stage", issue: 90, title: "Built", stage: "claimed" },
  { t: at(7), run: old, kind: "pr", issue: 90, pr: "130" },
  { t: at(8), run: old, kind: "done", issue: 90, outcome: "pr opened" },
  // This run.
  { t: at(10), run, kind: "run-start", slots: 3 },
  { t: at(11), run, kind: "stage", issue: 105, title: "Auth schema", stage: "claimed" },
  { t: at(12), run, kind: "stage", issue: 105, title: "Auth schema", stage: "implement" },
  { t: at(20), run, kind: "stage", issue: 105, title: "Auth schema", stage: "ci" },
  { t: at(21), run, kind: "pr", issue: 105, pr: "121" },
  { t: at(21), run, kind: "ci", issue: 105, url: `${repo}/actions/runs/9` },
  { t: at(22), run, kind: "waiting", list: [
    { issue: 108, queue: "build", why: "blocked by #105 (open)" },
    { issue: 112, queue: "triage", why: "waits for a slot" },
    { issue: 105, queue: "babysit", why: "CI fix waits for a slot" },
  ] },
];

const md = render(events, repo, new Date(at(30)));
console.log(md);
assert.match(md, /\*\*Ended:\*\* —/);
assert.match(md, /1 in flight · 3 slots/);
assert.match(md, /\| \[#105\]\(https:\/\/github.com\/o\/r\/issues\/105\) \| Auth schema \| ci \| .* \(10m\) \| \[#121\]/);
assert.doesNotMatch(md, /#80\]/, "a killed run's leftovers are not in flight");
assert.match(md, /\| build \| \[#108\]\(.*\) \| blocked by #105 \(open\) \|/);
assert.match(md, /\| babysit \| \[#105\][^\n]*\n\| build \| \[#108\][^\n]*\n\| triage \| \[#112\]/, "babysit first, triage last");
assert.match(md, /\| \[#73\]\(.*\) \| ✗ crashed \| \[#116\]\(.*\) \| 8m \| .* \| crashed at ci: push rejected; 2× rebase \|/);

assert.match(md, /\| \[#90\]\(.*\) \| → pr opened \| \[#130\]\(.*\) \| 3m \|/);

const ended = render([...events, { t: at(40), run, kind: "run-end", reason: "queue empty" }], repo, new Date(at(41)));
assert.match(ended, /\*\*Ended:\*\* \d\d:\d\d · queue empty/);
assert.doesNotMatch(ended, /\[#105\]\(.*\) \| Auth schema \| ci/, "nothing is in flight once the run ended");
assert.doesNotMatch(ended, /#108/, "waiting is cleared once the run ended");
console.log("status.ts: ok");
