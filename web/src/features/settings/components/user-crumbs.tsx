import { Link } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";

import { sectionTitles } from "@/features/settings/lib/sections";

/** The way back from the User called `name` to the list of Users: "Users › <name>". */
export function UserCrumbs({ name }: { name: string }) {
  const back = (
    <Link
      to="."
      search={(rest) => ({ ...rest, user: undefined })}
      // Else the router marks it current too, as the list's address is part of this one.
      activeOptions={{ exact: true }}
      className="-ms-1 flex min-h-11 items-center rounded-sm px-1 text-muted-foreground outline-none hover:text-foreground hover:underline focus-visible:ring-3 focus-visible:ring-ring/50 md:min-h-8"
    >
      {sectionTitles.users}
    </Link>
  );
  const here = (
    <li className="flex min-w-0 items-center gap-1">
      <ChevronRight
        aria-hidden
        strokeWidth={1.5}
        className="size-4 shrink-0 text-muted-foreground rtl:rotate-180"
      />
      <span aria-current="page" className="truncate font-medium">
        {name}
      </span>
    </li>
  );
  return (
    <nav aria-label="Breadcrumb">
      <ol className="flex min-w-0 items-center gap-1 text-sm">
        <li className="shrink-0">{back}</li>
        {here}
      </ol>
    </nav>
  );
}
