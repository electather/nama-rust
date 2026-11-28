import { Link } from "@tanstack/react-router";
import { ChevronLeft } from "lucide-react";

import { useOpenSection } from "@/features/settings/hooks/open-section";
import { listSearch } from "@/features/settings/lib/needs-a-match-search";

/** "‹ Needs a Match", back to the list, which keeps the Library `inLibrary` filtered to. */
export function BackToList({ inLibrary }: { inLibrary: string | undefined }) {
  const { title } = useOpenSection();
  return (
    <nav aria-label="Back" className="flex h-11 items-center pt-2 max-md:pt-0">
      <Link
        to="."
        search={(rest) => ({ ...rest, ...listSearch(inLibrary) })}
        aria-label={`Back to ${title}`}
        className="-ms-1 inline-flex h-11 items-center gap-0.5 rounded-full ps-1 pe-2.5 text-sm font-medium text-foreground outline-none hover:text-muted-foreground focus-visible:ring-3 focus-visible:ring-ring/50 md:h-9"
      >
        <ChevronLeft aria-hidden strokeWidth={1.5} className="size-5 rtl:rotate-180" />
        {title}
      </Link>
    </nav>
  );
}
