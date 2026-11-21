import type { PluginLogLine } from "@/gen/nama/v1/plugin_pb";
import { LogLine } from "@/shared/components/log-line";

/** Plugin Log lines in the order given, named by the heading `labelledBy`, or `empty` without any. */
export function LogLines({
  lines,
  labelledBy,
  empty,
}: {
  lines: PluginLogLine[];
  labelledBy: string;
  empty: string;
}) {
  if (lines.length === 0) {
    return <p className="mt-4 text-sm text-muted-foreground">{empty}</p>;
  }
  return (
    <ol aria-labelledby={labelledBy} className="mt-4 divide-y text-sm">
      {lines.map((line, index) => (
        // oxlint-disable-next-line react/no-array-index-key -- two lines may match in every field, and the list is replaced whole, never reordered
        <LogLine key={index} line={line} />
      ))}
    </ol>
  );
}
