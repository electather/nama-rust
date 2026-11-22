import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "nama-ui/components/button";
import type { RefObject } from "react";

const ways = {
  left: { Icon: ChevronLeft, by: -0.6 },
  right: { Icon: ChevronRight, by: 0.6 },
};

/**
 * A phone's arrow at one end of the `strip` of sections, which scrolls it that way. It only
 * scrolls: picking a section is what opens it.
 */
export function ScrollSections({
  strip,
  to,
}: {
  strip: RefObject<HTMLElement | null>;
  to: keyof typeof ways;
}) {
  const { Icon, by } = ways[to];
  return (
    <span className="flex shrink-0 md:hidden">
      <Button
        variant="ghost"
        size="round"
        aria-label={`Scroll sections ${to}`}
        onClick={() => {
          strip.current?.scrollBy({ left: by * strip.current.clientWidth });
        }}
      >
        <Icon aria-hidden strokeWidth={1.5} />
      </Button>
    </span>
  );
}
