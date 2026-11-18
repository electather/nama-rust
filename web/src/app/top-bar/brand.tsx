import { Link } from "@tanstack/react-router";

import { Logo } from "@/shared/components/logo";

/** The mark and name, linking Home. The mark shrinks a little once the bar lifts. */
export function Brand() {
  return (
    <Link
      to="/"
      className="inline-flex shrink-0 items-center gap-2.5 rounded-sm text-title text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
    >
      <Logo
        size={32}
        className="origin-left transition-transform duration-300 ease-out-expo group-data-lifted:scale-92 motion-reduce:transition-none"
      />
      Nama
    </Link>
  );
}
