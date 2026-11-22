import { Link } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";

import type { BoxSet } from "@/gen/nama/v1/browse_pb";
import { titleParam } from "@/shared/lib/title-param";

/** A Box Set row's heading: its name, opening the Box Set's page. */
export function BoxSetHeading({ boxSet, id }: { boxSet: BoxSet; id: string }) {
  return (
    <h2 id={id} className="text-lg font-semibold">
      <Link
        to="/box-sets/$boxSetId"
        params={{ boxSetId: titleParam(boxSet.id) }}
        className="inline-flex items-center gap-1 rounded-sm outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        {boxSet.name}
        <ChevronRight aria-hidden className="size-5 text-muted-foreground" />
      </Link>
    </h2>
  );
}
