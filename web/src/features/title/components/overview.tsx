import { Button, ghostHang } from "nama-ui/components/button";
import { useCallback, useState } from "react";

import { afterFontsLoad } from "@/shared/lib/after-fonts-load";

/** A title's overview in two lines, with "More" to show the rest, when it doesn't fit. */
export function Overview({ text }: { text: string }) {
  const [clamped, setClamped] = useState(false);
  const [open, setOpen] = useState(false);
  const measure = useCallback(
    (paragraph: HTMLParagraphElement | null) => {
      const update = () => {
        // Open, nothing is clamped, but "Less" must stay.
        if (paragraph && !open) {
          setClamped(paragraph.scrollHeight > paragraph.clientHeight);
        }
      };
      update();
      // Again when its width changes, and once the web font loads, which changes how many lines
      // the text takes but not the clamped box's size.
      const observer = new ResizeObserver(update);
      if (paragraph) {
        observer.observe(paragraph);
      }
      const stopWaiting = afterFontsLoad(update);
      return () => {
        stopWaiting();
        observer.disconnect();
      };
    },
    [open],
  );
  return (
    <div>
      <p
        ref={measure}
        className="text-pretty data-clamped:line-clamp-2"
        data-clamped={open ? undefined : ""}
      >
        {text}
      </p>
      {clamped && (
        // It stays once open, so keyboard focus isn't dropped.
        <div className={ghostHang}>
          <Button
            variant="ghost"
            aria-expanded={open}
            onClick={() => {
              setOpen(!open);
            }}
          >
            {open ? "Less" : "More"}
          </Button>
        </div>
      )}
    </div>
  );
}
