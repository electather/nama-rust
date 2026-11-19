import { ChevronRight } from "lucide-react";

import { Reasons } from "@/features/player/components/reasons";

/** The reasons that are only context, folded away until opened. */
export function MoreReasons({ texts }: { texts: string[] }) {
  if (texts.length === 0) {
    return null;
  }
  return (
    <details className="group/more mt-2.5">
      <summary className="inline-flex cursor-pointer list-none items-center gap-1 rounded-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/50 [&::-webkit-details-marker]:hidden">
        More
        <ChevronRight
          aria-hidden
          className="size-4 transition-transform duration-150 ease-out-expo group-open/more:rotate-90 motion-reduce:transition-none"
        />
      </summary>
      <Reasons texts={texts} />
    </details>
  );
}
