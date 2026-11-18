import { Link, type LinkProps } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { buttonVariants } from "@/shared/ui/button";

/** A link in the app chrome's bar, lit while its page is open. */
export function NavLink(props: LinkProps & { children: ReactNode }) {
  return (
    <Link
      className={buttonVariants({
        variant: "ghost",
        size: "bar",
        className: "aria-[current=page]:text-foreground",
      })}
      {...props}
    />
  );
}
