import type { ReactNode } from "react";

/** The titles of one kind found, under their `heading`, or nothing when `count` is none. */
export function FoundSection({
  heading,
  count,
  children,
}: {
  heading: string;
  count: number;
  children: ReactNode;
}) {
  if (count === 0) {
    return null;
  }
  return (
    <section aria-label={heading} className="mt-10">
      <h2 className="font-medium">{heading}</h2>
      {children}
    </section>
  );
}
