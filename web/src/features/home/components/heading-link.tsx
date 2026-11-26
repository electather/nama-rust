import { Link, type LinkOptions } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";
import type { ReactNode } from "react";

/** A row's heading text as a link to `link`, such as the Library whose titles the row lists. */
export function HeadingLink({ link, children }: { link: LinkOptions; children: ReactNode }) {
  return (
    <Link
      {...link}
      className="group inline-flex items-center gap-1 rounded-md outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      {children}
      <ChevronRight
        aria-hidden
        className="size-6 text-muted-foreground transition-transform group-hover:translate-x-0.5 motion-reduce:transition-none rtl:rotate-180 rtl:group-hover:-translate-x-0.5"
      />
    </Link>
  );
}
