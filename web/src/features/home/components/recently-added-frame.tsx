import { Link } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";
import type { ReactNode } from "react";

import { HomeSection } from "@/features/home/components/home-section";
import { SectionHeading } from "@/features/home/components/section-heading";
import type { RecentlyAddedHomeRow } from "@/gen/nama/v1/browse_pb";

/** A Library's Recently Added row under its heading, which opens the Library newest first. */
export function RecentlyAddedFrame({
  row,
  headingId,
  children,
}: {
  row: RecentlyAddedHomeRow;
  headingId: string;
  children: ReactNode;
}) {
  const heading = (
    <SectionHeading id={headingId}>
      <Link
        to="/libraries/$libraryId"
        params={{ libraryId: String(row.libraryId) }}
        search={{ sort: "added" }}
        className="group inline-flex items-center gap-1 rounded-md outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        Recently added in {row.libraryName}
        <ChevronRight
          aria-hidden
          className="size-6 text-muted-foreground transition-transform group-hover:translate-x-0.5 motion-reduce:transition-none rtl:rotate-180 rtl:group-hover:-translate-x-0.5"
        />
      </Link>
    </SectionHeading>
  );
  return (
    <HomeSection labelledBy={headingId} heading={heading}>
      {children}
    </HomeSection>
  );
}
