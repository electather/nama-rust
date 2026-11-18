import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { Page } from "@/shared/components/page";
import { buttonVariants } from "@/shared/ui/button";

/** The Request page's heading and the way back to all Requests, around `children`. */
export function RequestLayout({ children }: { children: ReactNode }) {
  return (
    <Page heading="Request">
      {/* Pulled back by the button's own padding, so its words line up with the heading. */}
      <div className="-ms-2.5 mt-2">
        <Link to="/requests" className={buttonVariants({ variant: "ghost" })}>
          All Requests
        </Link>
      </div>
      {children}
    </Page>
  );
}
