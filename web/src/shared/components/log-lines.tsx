import { useState } from "react";

import type { PluginLogLevel, PluginLogLine } from "@/gen/nama/v1/plugin_pb";
import { LogFilter } from "@/shared/components/log-filter";
import { LogLine } from "@/shared/components/log-line";

/**
 * Plugin Log lines in the order given, named by the heading `labelledBy`, or `empty` without any:
 * a filter by level over the lines, which scroll in a panel of their own once there are many.
 */
export function LogLines({
  lines,
  labelledBy,
  empty,
}: {
  lines: PluginLogLine[];
  labelledBy: string;
  empty: string;
}) {
  const [hidden, setHidden] = useState<PluginLogLevel[]>([]);
  if (lines.length === 0) {
    return <p className="mt-4 text-sm text-muted-foreground empty:hidden">{empty}</p>;
  }
  const shown = lines.filter((line) => !hidden.includes(line.level));
  const list = (
    <ol aria-labelledby={labelledBy} className="divide-y text-sm">
      {shown.map((line, index) => (
        // oxlint-disable-next-line react/no-array-index-key -- two lines may match in every field, and the list is replaced whole, never reordered
        <LogLine key={index} line={line} />
      ))}
    </ol>
  );
  return (
    <div className="mt-4">
      <LogFilter
        lines={lines}
        shown={shown.length}
        hidden={hidden}
        onToggle={(level) => {
          // From the latest, so two presses in a row can't undo one another.
          setHidden((now) =>
            now.includes(level) ? now.filter((each) => each !== level) : [...now, level],
          );
        }}
      />
      <div
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- a panel that scrolls must take focus for keyboard users to scroll it (WCAG 2.1.1)
        tabIndex={0}
        className="max-h-96 overflow-y-auto overscroll-contain border-t pe-2 outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        {list}
        <p className="py-3 text-sm text-muted-foreground empty:hidden">
          {shown.length === 0 && "No lines at the levels shown."}
        </p>
      </div>
    </div>
  );
}
