import type { PluginLogLevel, PluginLogLine } from "@/gen/nama/v1/plugin_pb";
import { levels } from "@/shared/lib/log-levels";
import { Button } from "@/shared/ui/button";

/**
 * What narrows a log: a button per level `lines` has, lit while its lines show, with how many it
 * has, and at the end how many lines show of them all.
 */
export function LogFilter({
  lines,
  shown,
  hidden,
  onToggle,
}: {
  lines: PluginLogLine[];
  /** How many of `lines` show. */
  shown: number;
  hidden: PluginLogLevel[];
  onToggle: (level: PluginLogLevel) => void;
}) {
  const counted = [...levels].flatMap(([level, { label }]) => {
    const count = lines.filter((line) => line.level === level).length;
    return count === 0 ? [] : [{ level, label, count }];
  });
  return (
    <fieldset className="flex min-w-0 flex-wrap items-center gap-1.5 pb-3">
      <legend className="sr-only">Levels shown</legend>
      {counted.map(({ level, label, count }) => (
        <Button
          key={level}
          variant="secondary"
          aria-pressed={!hidden.includes(level)}
          onClick={() => {
            onToggle(level);
          }}
        >
          {label}
          <span className="tabular-nums opacity-70">{count}</span>
        </Button>
      ))}
      <output className="ms-auto ps-1.5 text-xs text-muted-foreground tabular-nums">
        {shown} of {lines.length} lines
      </output>
    </fieldset>
  );
}
