import { Lock } from "lucide-react";

/** Why the Backup folder needs the care the data folder gets. */
export function KeepSafe() {
  return (
    <p className="flex items-start gap-1.5 text-xs text-pretty text-muted-foreground">
      <Lock aria-hidden strokeWidth={1.5} className="mt-px size-3.5 shrink-0" />
      Backups hold every Plugin’s API keys. Keep the folder as safe as the data folder.
    </p>
  );
}
