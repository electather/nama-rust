import type { LinkedServer } from "@hub/gen/nama/hub/v1/link_pb";

const linkedOn = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

/** One Linked Server: its name and when it was Linked. */
export function LinkedServerItem({ server }: { server: LinkedServer }) {
  return (
    <li className="flex items-baseline justify-between gap-3 py-2.5">
      <span className="min-w-0 text-base wrap-break-word">{server.name}</span>
      <span className="shrink-0 text-xs text-muted-foreground">
        Linked {linkedOn.format(new Date(Number(server.linkedAt) * 1000))}
      </span>
    </li>
  );
}
