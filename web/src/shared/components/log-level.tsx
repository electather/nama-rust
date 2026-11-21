import type { PluginLogLevel } from "@/gen/nama/v1/plugin_pb";
import { levels } from "@/shared/lib/log-levels";

/** A log line's level as an icon and a word, so it never rests on colour alone. Nothing for none. */
export function LogLevel({ level }: { level: PluginLogLevel }) {
  const shown = levels.get(level);
  if (!shown) {
    return null;
  }
  return (
    <span className={`flex items-center gap-1 ${shown.tone}`}>
      <shown.Icon aria-hidden strokeWidth={1.5} className="size-3.5" />
      {shown.label}
    </span>
  );
}
