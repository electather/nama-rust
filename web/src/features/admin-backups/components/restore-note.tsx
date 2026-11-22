import { CodeText } from "@/features/admin-backups/components/code-text";

/** How to restore a Backup, which only the CLI does, with the Server stopped (ADR 0068). */
export function RestoreNote({ folder, newest }: { folder: string; newest: string | undefined }) {
  return (
    <div className="flex max-w-prose flex-col gap-2 py-3.5 text-sm text-pretty">
      <p className="text-muted-foreground">
        Stop the Server, then run this on its machine with the Backup to restore, and start it
        again. Each file it replaces is kept beside it, with .before-restore added to its name.
      </p>
      <p>
        <CodeText>{`nama-server restore ${folder}/${newest ?? "nama-….tar"}`}</CodeText>
      </p>
      <p className="text-muted-foreground">
        In Docker, run it with docker run --rm, the Server’s image and the same volumes, while the
        Server’s container is stopped.
      </p>
    </div>
  );
}
