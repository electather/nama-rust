import { useEffect, useRef, useState } from "react";

/**
 * Whether the element `watch` is given has come within a screen of the view, which stays true
 * once it has, so a section below the fold asks the Server for its titles only as you near it.
 */
export function useNearScreen() {
  const watch = useRef<HTMLDivElement>(null);
  const [near, setNear] = useState(false);
  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setNear(true);
        }
      },
      { rootMargin: "100% 0px" },
    );
    if (watch.current) {
      observer.observe(watch.current);
    }
    return () => {
      observer.disconnect();
    };
  }, []);
  return { near, watch };
}
