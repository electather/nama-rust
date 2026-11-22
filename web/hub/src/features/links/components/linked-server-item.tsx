import type { LinkedServer } from "@hub/gen/nama/hub/v1/link_pb";

const linkedOn = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

/** One Linked Server: its name, linking to its Server URL if its Admin set one, and when it was Linked. */
export function LinkedServerItem({ server }: { server: LinkedServer }) {
  return (
    <li className="flex items-baseline justify-between gap-3 py-2.5">
      {server.url === "" ? (
        <span className="min-w-0 text-base wrap-break-word">{server.name}</span>
      ) : (
        // The Server serves its own web app; the Hub only links there (ADR 0010).
        <a
          href={server.url}
          className="min-w-0 text-base wrap-break-word text-primary underline underline-offset-3"
        >
          {server.name}
        </a>
      )}
      <span className="shrink-0 text-xs text-muted-foreground">
        Linked {linkedOn.format(new Date(Number(server.linkedAt) * 1000))}
      </span>
    </li>
  );
}
