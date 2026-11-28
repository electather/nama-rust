import { useEffect, useRef, useState } from "react";

/**
 * Whether the element `watch` is given has come into view, which stays true once it has, so
 * something can play its entrance once per mount. Seen at once where nothing can watch, as in a
 * browser without `IntersectionObserver`.
 */
export function useSeenOnce() {
  const watch = useRef<HTMLElement>(null);
  const [seen, setSeen] = useState(typeof IntersectionObserver === "undefined");
  useEffect(() => {
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        setSeen(true);
      }
    });
    if (!seen && watch.current) {
      observer.observe(watch.current);
    }
    return () => {
      observer.disconnect();
    };
  }, [seen]);
  return { seen, watch };
}
