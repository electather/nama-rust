import type { ServerLink } from "@/gen/nama/v1/server_pb";

/** The names the Hub gave the Linked Server, its own first; nothing until it named it. */
export function ServerNames({ link }: { link: Pick<ServerLink, "names"> }) {
  if (link.names.length === 0) {
    return null;
  }
  return (
    <div className="text-sm text-muted-foreground">
      <p id="server-names">The Hub’s names for the Server:</p>
      <ul aria-labelledby="server-names" className="wrap-anywhere">
        {link.names.map((name) => (
          <li key={name}>{name}</li>
        ))}
      </ul>
    </div>
  );
}
