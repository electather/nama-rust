import type { PluginLogLine } from "@/gen/nama/v1/plugin_pb";
import { LogLevel } from "@/shared/components/log-level";

/**
 * One line a Plugin logged: when and at what level, over what it says. The time and the text are in
 * monospace, DESIGN.md's one use of it, as they're read as data.
 */
export function LogLine({ line }: { line: PluginLogLine }) {
  const at = new Date(Number(line.atMs));
  return (
    <li className="flex flex-col gap-0.5 py-2.5">
      <p className="flex flex-wrap items-center gap-x-2.5 text-xs text-muted-foreground">
        <time dateTime={at.toISOString()} className="font-mono">
          {at.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "medium" })}
        </time>
        <LogLevel level={line.level} />
      </p>
      <p className="font-mono wrap-break-word whitespace-pre-wrap">{line.text}</p>
    </li>
  );
}
