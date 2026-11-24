import { Link } from "@tanstack/react-router";
import { cn } from "cn";
import { buttonVariants, ghostHang } from "nama-ui/components/button";

/** The way out of a page that can't show what it's for. */
export function BackToLibraries() {
  return (
    <div className={cn(ghostHang, "mt-6")}>
      <Link to="/" className={buttonVariants({ variant: "ghost" })}>
        Back to your Libraries
      </Link>
    </div>
  );
}
