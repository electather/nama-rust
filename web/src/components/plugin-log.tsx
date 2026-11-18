import { useQuery } from "@connectrpc/connect-query";
import { useRouteContext } from "@tanstack/react-router";

import { PluginLogLevel, type PluginLogLine, PluginService } from "@/gen/nama/v1/plugin_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/lib/permissions";

const levels: Record<PluginLogLevel, string> = {
  [PluginLogLevel.UNSPECIFIED]: "",
  [PluginLogLevel.DEBUG]: "Debug",
  [PluginLogLevel.INFO]: "Info",
  [PluginLogLevel.WARN]: "Warning",
  [PluginLogLevel.ERROR]: "Error",
};

/**
 * For an Admin, the last lines the Plugin `id` logged, newest first, as the Server keeps them while
 * it's enabled.
 */
export function PluginLog({ id }: { id: string }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return may(signedIn.user, Permission.ADMIN) && <LogSection id={id} />;
}

function LogSection({ id }: { id: string }) {
  const listed = useQuery(PluginService.method.listPluginLogLines, { id });
  return (
    <section aria-labelledby="plugin-log" className="mt-10">
      <h2 id="plugin-log" className="font-medium">
        Plugin Log
      </h2>
      <p className="mt-1 max-w-prose text-sm text-pretty text-muted-foreground">
        Its last 100 lines, newest first, kept until it’s disabled or the Server restarts.
      </p>
      {listed.data && <LogLines lines={listed.data.lines} />}
      <p role="alert" className="mt-4 text-sm text-destructive empty:hidden">
        {listed.error && "Couldn’t load the Plugin Log. Check that the Server is running."}
      </p>
    </section>
  );
}

function LogLines({ lines }: { lines: PluginLogLine[] }) {
  if (lines.length === 0) {
    return (
      <p className="mt-4 text-sm text-muted-foreground">Nothing logged since it was enabled.</p>
    );
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

function LogLine({ line }: { line: PluginLogLine }) {
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
