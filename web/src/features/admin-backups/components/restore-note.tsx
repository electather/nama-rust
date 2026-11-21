import { CodeText } from "@/features/admin-backups/components/code-text";

/** How to restore a Backup, which only the CLI does, with the Server stopped (ADR 0068). */
export function RestoreNote({ folder, newest }: { folder: string; newest: string | undefined }) {
  return (
    <section aria-labelledby="restoring" className="mt-12 flex max-w-prose flex-col gap-3">
      <h2 id="restoring" className="font-medium">
        Restoring a Backup
      </h2>
      <p className="text-pretty text-muted-foreground">
        Stop the Server, then run this on its machine with the Backup to restore, and start it
        again. Each file it replaces is kept beside it, with .before-restore added to its name.
      </p>
      <p>
        <CodeText>{`nama-server restore ${folder}/${newest ?? "nama-….tar"}`}</CodeText>
      </p>
      <p className="text-pretty text-muted-foreground">
        In Docker, run it with docker run --rm, the Server’s image and the same volumes, while the
        Server’s container is stopped.
      </p>
    </section>
  );
}
