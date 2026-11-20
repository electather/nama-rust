import { SetupRun } from "@/features/admin-plugins/components/setup-run";
import type { SetupRun as Run } from "@/features/admin-plugins/lib/setup-markdown";

/** One paragraph or list item of a Plugin's setup steps. */
export function SetupRuns({ runs }: { runs: Run[] }) {
  return runs.map((run, index) => (
    // oxlint-disable-next-line react/no-array-index-key -- runs may repeat, and the list is replaced whole, never reordered
    <SetupRun key={index} run={run} />
  ));
}
