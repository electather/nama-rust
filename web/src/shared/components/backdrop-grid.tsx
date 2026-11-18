import type { ReactNode } from "react";

/** A grid of `BackdropCard`s: one column on a phone, up to three on a desktop. */
export function BackdropGrid({ children }: { children: ReactNode }) {
  return <ul className="mt-4 grid gap-x-4 gap-y-6 sm:grid-cols-2 lg:grid-cols-3">{children}</ul>;
}
