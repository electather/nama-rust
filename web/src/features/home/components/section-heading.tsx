import type { ReactNode } from "react";

/** A Home section's heading, which names its section. */
export function SectionHeading({ id, children }: { id: string; children: ReactNode }) {
  return (
    <h2 id={id} className="text-section font-semibold text-balance">
      {children}
    </h2>
  );
}
