import type { ReactNode } from "react";

/** A Home section, named by its heading, `heading`, over its shelf. */
export function HomeSection({
  labelledBy,
  heading,
  children,
}: {
  labelledBy: string;
  heading: ReactNode;
  children: ReactNode;
}) {
  return (
    <section aria-labelledby={labelledBy} className="flex min-w-0 flex-col gap-5">
      {heading}
      <div className="-mt-1">{children}</div>
    </section>
  );
}
