import { RestoreCommand } from "@/features/settings/components/restore-command";

// Numbered, the steps either side of the command that sits between them.
const STEPS = "flex list-decimal flex-col gap-2 ps-4.5 text-sm";

/**
 * How to restore the Backup `name` in `folder`, under its row: only the CLI restores, with the
 * Server stopped (ADR 0068), so this is the command to run and what happens around it.
 */
export function RestoreSteps({ id, folder, name }: { id: string; folder: string; name: string }) {
  return (
    <li id={id} className="ms-9 mb-2.5 flex flex-col gap-2 rounded-xl border bg-card px-3.5 py-3">
      <ol className={STEPS}>
        <li>Stop the Server.</li>
        <li>On its machine, run:</li>
      </ol>
      <RestoreCommand command={`nama-server restore ${folder}/${name}`} />
      <ol start={3} className={STEPS}>
        <li>Start it again. Each file it replaced is kept beside it, ending in .before-restore.</li>
      </ol>
      <p className="text-xs text-pretty text-muted-foreground">
        In Docker, run it with docker run --rm, the Server’s image and the same volumes, while its
        container is stopped.
      </p>
    </li>
  );
}
