import { useEffect, useRef } from "react";

import { Button } from "@/components/ui/button";

/**
 * A "Show more" button that calls `load`, an infinite query's `fetchNextPage`, when pressed or
 * when it scrolls into view. `loaded` is how many pages have loaded.
 */
export function ShowMore({
  load,
  loaded,
}: {
  load: (options: { cancelRefetch: boolean }) => Promise<unknown>;
  loaded: number;
}) {
  const button = useRef<HTMLButtonElement>(null);
  // Observes again once each page loads: a page short enough to leave the button in view fires no
  // new intersection, and a new observer reports the button's visibility straight away.
  useEffect(() => {
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) button.current?.click();
    });
    if (button.current) observer.observe(button.current);
    return () => {
      observer.disconnect();
    };
    // oxlint-disable-next-line react/exhaustive-effect-dependencies -- `loaded` is what re-runs it; keying the button to remount would lose a keyboard User's focus
  }, [loaded]);
  return (
    <div className="mt-2 -ms-4">
      <Button
        ref={button}
        variant="ghost"
        onClick={() => {
          // Scrolling and pressing at once must not ask for the same page twice.
          void load({ cancelRefetch: false });
        }}
      >
        Show more
      </Button>
    </div>
  );
}
