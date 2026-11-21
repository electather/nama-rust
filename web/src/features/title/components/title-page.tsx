import type { ReactNode } from "react";

/** A Movie's or Show's page: its `hero` at the top, then `children`, its sections, in the column. */
export function TitlePage({ hero, children }: { hero: ReactNode; children: ReactNode }) {
  return (
    <main className="overflow-x-clip pb-16">
      {hero}
      <div className="mx-auto mt-8 flex w-full max-w-350 flex-col gap-12 px-5">{children}</div>
    </main>
  );
}
