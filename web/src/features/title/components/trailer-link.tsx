import { ExternalLink } from "lucide-react";

import type { Trailer } from "@/gen/nama/v1/request_pb";
import { buttonVariants } from "@/shared/ui/button";

/** One of a title's Trailers, as a list item linking out to it in a new tab. */
export function TrailerLink({ trailer }: { trailer: Trailer }) {
  return (
    <li className="max-w-full">
      <a
        href={trailer.url}
        target="_blank"
        rel="noopener noreferrer"
        className={buttonVariants({ variant: "secondary", size: "tall", className: "max-w-full" })}
      >
        {/* A source's names run long ("Alien | 40th Anniversary Trailer | 20th Century FOX"). */}
        <span className="truncate">{trailer.name}</span>
        <span className="sr-only"> (opens in a new tab)</span>
        <ExternalLink aria-hidden="true" className="shrink-0" />
      </a>
    </li>
  );
}
