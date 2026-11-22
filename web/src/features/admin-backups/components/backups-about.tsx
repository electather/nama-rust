import { CodeText } from "@/features/admin-backups/components/code-text";

/** What a Backup holds, why to keep it safe, and the `folder` Backups are written to. */
export function BackupsAbout({ folder }: { folder: string }) {
  return (
    <div className="flex max-w-prose flex-col gap-2 text-sm text-pretty">
      <p className="text-muted-foreground">
        A Backup holds a copy of the Server’s database, its config file and its Custom Images, all a
        restore needs to put it back. It also holds every Plugin’s settings, API keys included, so
        keep the Backup folder as safe as the Server’s data folder.
      </p>
      <p>
        Backups are written to <CodeText>{folder}</CodeText>
      </p>
    </div>
  );
}
