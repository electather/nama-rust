import { Link, useParams, useSearch } from "@tanstack/react-router";
import { cn } from "cn";
import { ChevronDown } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuLinkItem,
  DropdownMenuTrigger,
} from "nama-ui/components/dropdown-menu";

import type { LibraryGroup } from "@/app/data/use-top-bar-libraries";

/**
 * The top bar's Library links folded into one menu, for a phone, which has no room for them, or
 * for two or more of `groups`, the Hub's Servers, at every width under each Server's name. It's
 * named by the Library open, if one is.
 */
export function LibrariesMenu({ groups }: { groups: LibraryGroup[] }) {
  const label = useOpenLibraryName(groups) ?? "Libraries";
  if (groups.every(({ libraries }) => libraries.length === 0)) {
    return null;
  }
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <button
            type="button"
            aria-label={label}
            className={cn(
              "ms-1 inline-flex h-8 shrink-0 items-center gap-1 rounded-lg px-2.5 text-sm text-foreground outline-none hover:bg-muted/50 focus-visible:ring-3 focus-visible:ring-ring/50",
              groups.length === 1 && "sm:hidden",
            )}
          />
        }
      >
        {label}
        <ChevronDown aria-hidden strokeWidth={1.5} className="size-3.5" />
      </DropdownMenuTrigger>
      <DropdownMenuContent>{menuItems(groups)}</DropdownMenuContent>
    </DropdownMenu>
  );
}

/** The name of the Library open, if it's one of `groups`'. */
function useOpenLibraryName(groups: LibraryGroup[]) {
  const open = useParams({ strict: false, select: ({ libraryId }) => libraryId });
  const openOn = useSearch({ strict: false, select: ({ server }) => server });
  return groups
    .find(({ server }) => server?.id === openOn)
    ?.libraries.find(({ id }) => String(id) === open)?.name;
}

/** A link to each Library, under its Server's name for two or more Servers. */
function menuItems(groups: LibraryGroup[]) {
  return groups.length > 1
    ? groups.filter(({ libraries }) => libraries.length > 0).map((group) => serverItems(group))
    : groups.flatMap((group) => libraryItems(group));
}

/** `server`'s Libraries under its name. */
function serverItems(group: LibraryGroup) {
  return (
    <DropdownMenuGroup key={group.server?.id}>
      <DropdownMenuLabel>{group.server?.name}</DropdownMenuLabel>
      {libraryItems(group)}
    </DropdownMenuGroup>
  );
}

/** A link to each of `libraries`, on the Linked Server `server` at the Hub. */
function libraryItems({ server, libraries }: LibraryGroup) {
  return libraries.map(({ id, name }) => {
    const link = (
      <Link
        to="/libraries/$libraryId"
        params={{ libraryId: String(id) }}
        search={{ server: server?.id }}
      />
    );
    return (
      <DropdownMenuLinkItem key={String(id)} render={link}>
        {name}
      </DropdownMenuLinkItem>
    );
  });
}
