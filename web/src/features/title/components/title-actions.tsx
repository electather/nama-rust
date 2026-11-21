import type { ReactNode } from "react";

/** A title's actions in a row under its facts: Play or Request first, then the round buttons. */
export function TitleActions({ children }: { children: ReactNode }) {
  return <div className="mt-2.5 flex flex-wrap items-center gap-2.5">{children}</div>;
}
