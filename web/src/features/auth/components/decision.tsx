import type { ReactNode } from "react";

import { PageTitle } from "@/shared/components/page-title";

/** A centred heading and what it means, over two buttons side by side, the first on the end. */
export function Decision({
  heading,
  text,
  children,
}: {
  heading: string;
  text: ReactNode;
  /** The two buttons, primary first. */
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-5 text-center">
      <div className="flex flex-col gap-2">
        <PageTitle name={heading} />
        <h1 className="text-3xl font-medium tracking-tight text-balance wrap-anywhere">
          {heading}
        </h1>
        {text}
      </div>
      {/* Reversed, so on a phone the primary button wraps above the other. */}
      <div className="flex flex-row-reverse flex-wrap gap-2 *:grow *:basis-35">{children}</div>
    </div>
  );
}
