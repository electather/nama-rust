import { Link } from "@tanstack/react-router";

import { buttonVariants } from "@/shared/ui/button";

/** The way out of a page that can't show what it's for. */
export function BackToLibraries() {
  return (
    // Pulled back by the button's own padding, so its words line up with the text above.
    <div className="-ms-4 mt-6">
      <Link to="/" className={buttonVariants({ variant: "ghost" })}>
        Back to your Libraries
      </Link>
    </div>
  );
}
