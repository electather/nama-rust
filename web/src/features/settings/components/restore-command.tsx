import { CopyLinkButton } from "@/features/invites/components/copy-link-button";

/** The `command` that restores a Backup, on one line that scrolls sideways, and a button that copies it. */
export function RestoreCommand({ command }: { command: string }) {
  return (
    <span className="flex items-center gap-1.5 ps-4.5">
      <code className="min-w-0 flex-1 scrollbar-quiet overflow-x-auto rounded-lg bg-muted px-2 py-1.5 font-sans text-sm whitespace-nowrap">
        {command}
      </code>
      <CopyLinkButton url={command} label="Copy" />
    </span>
  );
}
