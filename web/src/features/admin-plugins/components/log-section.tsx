import { usePluginLogLines } from "@/features/admin-plugins/data/use-plugin-log-lines";
import { LogLines } from "@/shared/components/log-lines";

/** The Plugin Log heading and its lines, or why they didn't load. */
export function LogSection({ id }: { id: string }) {
  const listed = usePluginLogLines(id);
  return (
    <section aria-labelledby="plugin-log" className="mt-10">
      <h2 id="plugin-log" className="font-medium">
        Plugin Log
      </h2>
      <p className="mt-1 max-w-prose text-sm text-pretty text-muted-foreground">
        Its last 100 lines, newest first, kept until it’s disabled or the Server restarts.
      </p>
      {listed.data && (
        <LogLines lines={listed.data.lines} labelledBy="plugin-log" empty="Nothing logged yet." />
      )}
      <p role="alert" className="mt-4 text-sm text-destructive empty:hidden">
        {listed.error && "Couldn’t load the Plugin Log. Check that the Server is running."}
      </p>
    </section>
  );
}
