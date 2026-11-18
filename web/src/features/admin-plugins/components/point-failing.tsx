import { TriangleAlert } from "lucide-react";

import { failureMeans, points } from "@/features/admin-plugins/lib/extension-points";
import type { ExtensionPointFailure } from "@/gen/nama/v1/plugin_pb";
import { StateLabel } from "@/shared/components/state-label";

/** One Extension Point that keeps failing, with the last error. */
export function PointFailing({ failure }: { failure: ExtensionPointFailure }) {
  const { point, lastError, atMs, failedInARow } = failure;
  const shown = points[point];
  if (shown === undefined) {
    return null;
  }
  const at = new Date(Number(atMs));
  const calls = failedInARow === 1 ? "Its last call" : `Its last ${failedInARow} calls`;
  const heading = `failing-${point}`;
  return (
    <section aria-labelledby={heading} className="mt-6 max-w-prose">
      <h2 id={heading}>
        <StateLabel label={`${shown.label} failing`} Icon={TriangleAlert} />
      </h2>
      <p className="mt-2 text-sm text-pretty text-muted-foreground">
        {calls} failed, the latest on{" "}
        <time dateTime={at.toISOString()}>
          {at.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })}
        </time>
        . {failureMeans[point]}
      </p>
      <p className="mt-1 text-sm text-pretty wrap-break-word">Last error: {lastError}</p>
    </section>
  );
}
