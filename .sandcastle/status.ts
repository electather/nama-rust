// The factory's record of what it did: one JSON event per line in events.jsonl, rendered to status.md.
// Rendering replays the whole file, so the "Done" history survives across runs.
// ponytail: replays every event on each change; trim events.jsonl if it ever gets slow.
import { appendFileSync, readFileSync, writeFileSync } from "node:fs";

export type Stage = "claimed" | "implement" | "review" | "pr" | "ci" | "merging";
export type Outcome = "merged" | "handed back" | "cancelled" | "crashed" | "interrupted";
export type Waiting = { issue: number; why: string };

export type Event = { t: string; run: string } & (
  | { kind: "run-start"; slots: number }
  | { kind: "stage"; issue: number; title: string; stage: Stage }
  | { kind: "pr"; issue: number; pr: string }
  | { kind: "ci"; issue: number; url: string }
  | { kind: "note"; issue: number; text: string } // e.g. "rebase", "CI fix"
  | { kind: "done"; issue: number; outcome: Outcome; why?: string }
  | { kind: "waiting"; list: Waiting[] }
  | { kind: "run-end"; reason: string }
);
type Body = Event extends infer E ? (E extends Event ? Omit<E, "t" | "run"> : never) : never;

export function recorder(dir: string, run: string, repoUrl: string, now = () => new Date()) {
  const eventsFile = `${dir}/events.jsonl`;
  const statusFile = `${dir}/status.md`;
  return (body: Body) => {
    const event = { t: now().toISOString(), run, ...body } as Event;
    appendFileSync(eventsFile, `${JSON.stringify(event)}\n`);
    const events = readFileSync(eventsFile, "utf8").trim().split("\n").map((l) => JSON.parse(l) as Event);
    writeFileSync(statusFile, render(events, repoUrl, now()));
  };
}

const hhmm = (t: string) => new Date(t).toTimeString().slice(0, 5);
const mins = (from: string, to: string | Date) =>
  `${Math.round((new Date(to).getTime() - new Date(from).getTime()) / 60_000)}m`;
const cell = (s: string) => s.replaceAll("|", "\\|").replaceAll("\n", " ");

type Track = { title: string; stage: Stage; since: string; claimedAt: string; pr?: string; ci?: string; notes: string[] };

export function render(events: Event[], repoUrl: string, now: Date): string {
  const start = events.findLast((e) => e.kind === "run-start");
  const run = start?.run;
  const end = events.findLast((e) => e.kind === "run-end" && e.run === run);

  // Every issue's latest attempt, whichever run it was in.
  const tracks = new Map<number, Track>();
  const done: { issue: number; outcome: Outcome; why?: string; t: string; track?: Track }[] = [];
  for (const e of events) {
    if (e.kind === "stage") {
      const prev = tracks.get(e.issue);
      tracks.set(e.issue, e.stage === "claimed" || !prev
        ? { title: e.title, stage: e.stage, since: e.t, claimedAt: e.t, notes: [] }
        : { ...prev, stage: e.stage, since: e.t });
    } else if (e.kind === "pr" || e.kind === "ci" || e.kind === "note") {
      const tr = tracks.get(e.issue);
      if (!tr) continue;
      if (e.kind === "pr") tr.pr = e.pr;
      if (e.kind === "ci") tr.ci = e.url;
      if (e.kind === "note") tr.notes.push(e.text);
    } else if (e.kind === "done") {
      done.push({ issue: e.issue, outcome: e.outcome, why: e.why, t: e.t, track: tracks.get(e.issue) });
      tracks.delete(e.issue);
    }
  }
  // Only this run's issues are in flight; a killed run's leftovers are not.
  const claimedThisRun = new Set(
    events.filter((e) => e.kind === "stage" && e.stage === "claimed" && e.run === run).map((e) => (e as { issue: number }).issue),
  );
  const inFlight = end ? [] : [...tracks].filter(([n]) => claimedThisRun.has(n));
  const waiting = end ? [] : (events.findLast((e) => e.kind === "waiting" && e.run === run) as { list: Waiting[] } | undefined)?.list ?? [];

  const link = (kind: "issues" | "pull", n: string | number) => `[#${n}](${repoUrl}/${kind}/${n})`;
  const icon = { merged: "✓", "handed back": "⚠", cancelled: "⊘", crashed: "✗", interrupted: "⏸" };
  const counts = (notes: string[]) =>
    [...new Set(notes)].map((n) => `${notes.filter((x) => x === n).length}× ${n}`).join(", ");

  const out = [`# Factory status · updated ${now.toTimeString().slice(0, 8)}`, ""];
  if (start?.kind === "run-start") {
    // In flight counts PRs waiting for CI or to merge, which hold no slot (see slotPool in main.ts).
    out.push(`**Run:** ${run} · started ${hhmm(start.t)} · ${inFlight.length} in flight · ${start.slots} slots`);
    out.push(`**Ended:** ${end?.kind === "run-end" ? `${hhmm(end.t)} · ${cell(end.reason)}` : "—"}`, "");
  }
  out.push("## In flight", "| Issue | Title | Stage | Since | PR | CI |", "|---|---|---|---|---|---|");
  for (const [n, tr] of inFlight) {
    out.push(`| ${link("issues", n)} | ${cell(tr.title)} | ${tr.stage} | ${hhmm(tr.since)} (${mins(tr.since, now)}) | ${tr.pr ? link("pull", tr.pr) : "—"} | ${tr.ci ? `[run](${tr.ci})` : "—"} |`);
  }
  out.push("", "## Waiting", "| Issue | Why |", "|---|---|");
  for (const w of waiting) out.push(`| ${link("issues", w.issue)} | ${cell(w.why)} |`);
  out.push("", "## Done (last 20)", "| Issue | Outcome | PR | Took | Finished | Note |", "|---|---|---|---|---|---|");
  for (const d of done.slice(-20).reverse()) {
    const tr = d.track;
    const note = [d.why, tr && counts(tr.notes)].filter(Boolean).join("; ");
    out.push(`| ${link("issues", d.issue)} | ${icon[d.outcome]} ${d.outcome} | ${tr?.pr ? link("pull", tr.pr) : "—"} | ${tr ? mins(tr.claimedAt, d.t) : "—"} | ${hhmm(d.t)} | ${cell(note || "")} |`);
  }
  return `${out.join("\n")}\n`;
}
