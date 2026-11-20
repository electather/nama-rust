import type { ReactNode } from "react";

import { useNearScreen } from "@/features/home/hooks/use-near-screen";

/**
 * `children` once this comes within a screen of the view, and `placeholder` until then. Once
 * near, its box steps aside, so a section that turns out empty leaves no gap in the page.
 */
export function NearScreen({
  placeholder,
  children,
}: {
  placeholder: ReactNode;
  children: ReactNode;
}) {
  const { near, watch } = useNearScreen();
  return (
    <div ref={watch} className={near ? "contents" : undefined}>
      {near ? children : placeholder}
    </div>
  );
}
