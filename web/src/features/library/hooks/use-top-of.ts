import { type RefObject, useLayoutEffect, useState } from "react";

/** Where the element `ref` holds starts, down the page, once it is laid out. */
export function useTopOf(ref: RefObject<HTMLElement | null>) {
  const [top, setTop] = useState(0);
  useLayoutEffect(() => {
    setTop((ref.current?.getBoundingClientRect().top ?? 0) + window.scrollY);
  }, [ref]);
  return top;
}
