import { Link, type LinkProps } from "@tanstack/react-router";
import type { ReactNode } from "react";

/** A link in the top bar, on a pill while its page is open. */
export function TopBarLink(props: LinkProps & { children: ReactNode; "aria-label"?: string }) {
  return (
    <Link
      className="inline-flex h-8 items-center rounded-lg border border-transparent px-3.5 text-sm whitespace-nowrap text-foreground transition-colors outline-none hover:bg-muted/50 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=page]:border-border aria-[current=page]:bg-secondary aria-[current=page]:font-medium"
      {...props}
    />
  );
}
