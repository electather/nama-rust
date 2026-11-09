import { useEffect, useRef } from "react";

import { Button } from "@/components/ui/button";

/**
 * A "Show more" button that calls `load`, an infinite query's `fetchNextPage`, when pressed or
 * when it scrolls into view.
 */
export function ShowMore({
  load,
}: {
  load: (options: { cancelRefetch: boolean }) => Promise<unknown>;
}) {
  const button = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) button.current?.click();
    });
    if (button.current) observer.observe(button.current);
    return () => {
      observer.disconnect();
    };
  }, []);
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
