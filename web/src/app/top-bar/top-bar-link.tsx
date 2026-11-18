import { Link, type LinkProps } from "@tanstack/react-router";
import type { ReactNode } from "react";

/** A link in the top bar, on a `secondary` pill while its page is open. */
export function TopBarLink(props: LinkProps & { "aria-label"?: string; children: ReactNode }) {
  return (
    <Link
      className="inline-flex h-8 items-center rounded-sm border border-transparent px-3.5 text-sm whitespace-nowrap text-foreground-soft transition-colors duration-150 ease-out-expo outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background aria-[current=page]:border-border aria-[current=page]:bg-secondary aria-[current=page]:font-medium aria-[current=page]:text-foreground motion-reduce:transition-none"
      {...props}
    />
  );
}
