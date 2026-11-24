import type { ReactNode } from "react";

import { PageTitle } from "@/shared/components/page-title";

/**
 * A Movie's, Show's or Box Set's page, `name`: its `hero` at the top, then `children`, its
 * sections, in the column.
 */
export function TitlePage({
  name,
  hero,
  children,
}: {
  name: string;
  hero: ReactNode;
  children: ReactNode;
}) {
  return (
    <main className="overflow-x-clip pb-16">
      <PageTitle name={name} />
      {hero}
      <div className="mx-auto mt-8 flex w-full max-w-350 flex-col gap-12 px-5">{children}</div>
    </main>
  );
}
