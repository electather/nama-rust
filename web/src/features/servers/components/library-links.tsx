import type { Transport } from "@connectrpc/connect";
import { Link } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";

import { useLibraries } from "@/shared/hooks/use-libraries";

/** The Libraries on the Linked Server `serverId`, which `transport` reaches, each opening it. */
export function LibraryLinks({ serverId, transport }: { serverId: string; transport: Transport }) {
  const libraries = useLibraries(transport);
  if (libraries.length === 0) {
    return <p className="text-sm text-muted-foreground">No Library is shared with you here.</p>;
  }
  return (
    <ul className="flex flex-wrap gap-2">
      {libraries.map((library) => (
        <li key={String(library.id)}>
          <Link
            to="/libraries/$libraryId"
            params={{ libraryId: String(library.id) }}
            search={{ server: serverId }}
            className={buttonVariants({ variant: "secondary" })}
          >
            {library.name}
          </Link>
        </li>
      ))}
    </ul>
  );
}
