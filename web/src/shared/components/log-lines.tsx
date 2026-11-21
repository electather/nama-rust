import { LogLine } from "@/features/admin-plugins/components/log-line";
import type { PluginLogLine } from "@/gen/nama/v1/plugin_pb";

/** The Plugin's log lines in the order given, or that nothing was logged. */
export function LogLines({ lines }: { lines: PluginLogLine[] }) {
  if (lines.length === 0) {
    return <p className="mt-4 text-sm text-muted-foreground">Nothing logged yet.</p>;
  }
  return (
    <ol aria-labelledby="plugin-log" className="mt-4 divide-y text-sm">
      {lines.map((line, index) => (
        // oxlint-disable-next-line react/no-array-index-key -- two lines may match in every field, and the list is replaced whole, never reordered
        <LogLine key={index} line={line} />
      ))}
    </ol>
  );
}
