import { usePluginLogLines } from "@/features/admin-plugins/data/use-plugin-log-lines";
import type { ListPluginLogLinesResponse } from "@/gen/nama/v1/plugin_pb";
import { LogLines } from "@/shared/components/log-lines";

/** The lines listed, none while they're still on their way. */
function linesOf(listed: ListPluginLogLinesResponse | undefined) {
  return listed?.lines ?? [];
}

/** Why the lines didn't load, if they didn't. */
function failure(failed: boolean) {
  return failed ? "Couldn’t load the Plugin Log. Check that the Server is running." : undefined;
}

/**
 * The Plugin Log heading and its lines, or why they didn't load. Nothing for a Plugin that has
 * logged nothing, or while its lines are still on their way.
 */
export function LogSection({ id }: { id: string }) {
  const listed = usePluginLogLines(id);
  const lines = linesOf(listed.data);
  if (lines.length === 0 && !listed.isError) {
    return null;
  }
  return (
    <section aria-labelledby="plugin-log" className="mt-10">
      <h2 id="plugin-log" className="font-medium">
        Plugin Log
      </h2>
      <p className="mt-1 max-w-prose text-sm text-pretty text-muted-foreground">
        Its last 100 lines, newest first, kept until it’s disabled or the Server restarts.
      </p>
      <LogLines lines={lines} labelledBy="plugin-log" empty="" />
      <p role="alert" className="mt-4 text-sm text-destructive empty:hidden">
        {failure(listed.isError)}
      </p>
    </section>
  );
}
