import { levels } from "@/features/admin-plugins/lib/log-levels";
import type { PluginLogLine } from "@/gen/nama/v1/plugin_pb";

/** One line the Plugin logged, with when and at what level. */
export function LogLine({ line }: { line: PluginLogLine }) {
  const at = new Date(Number(line.atMs));
  return (
    <li className="py-2">
      <p className="text-muted-foreground tabular-nums">
        <time dateTime={at.toISOString()}>
          {at.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "medium" })}
        </time>{" "}
        · {levels[line.level]}
      </p>
      <p className="wrap-break-word whitespace-pre-wrap">{line.text}</p>
    </li>
  );
}
