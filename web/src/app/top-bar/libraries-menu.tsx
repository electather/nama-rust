import { Link, useParams } from "@tanstack/react-router";
import { ChevronDown } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLinkItem,
  DropdownMenuTrigger,
} from "nama-ui/components/dropdown-menu";

import { useLoadedLibraries } from "@/shared/hooks/use-loaded-libraries";

/**
 * The top bar's Library links folded into one menu, for a phone, which has no room for them. It's
 * named by the Library open, if one is.
 */
export function LibrariesMenu() {
  const libraries = useLoadedLibraries();
  const open = useParams({ strict: false, select: ({ libraryId }) => libraryId });
  if (libraries.length === 0) {
    return null;
  }
  const label = libraries.find(({ id }) => String(id) === open)?.name ?? "Libraries";
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <button
            type="button"
            aria-label={label}
            className="ms-1 inline-flex h-8 shrink-0 items-center gap-1 rounded-lg px-2.5 text-sm text-foreground outline-none hover:bg-muted/50 focus-visible:ring-3 focus-visible:ring-ring/50 sm:hidden"
          />
        }
      >
        {label}
        <ChevronDown aria-hidden strokeWidth={1.5} className="size-3.5" />
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        {libraries.map(({ id, name }) => {
          const link = <Link to="/libraries/$libraryId" params={{ libraryId: String(id) }} />;
          return (
            <DropdownMenuLinkItem key={String(id)} render={link}>
              {name}
            </DropdownMenuLinkItem>
          );
        })}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
