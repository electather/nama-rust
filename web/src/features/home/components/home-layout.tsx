import type { ReactNode } from "react";

/**
 * Home's frame: Featured across the top, then its sections in the page's column, on a soft light
 * from above that starts under the top bar, so it has no seam where the bar ends.
 */
export function HomeLayout({ hero, children }: { hero: ReactNode; children: ReactNode }) {
  return (
    <main className="-mt-18 overflow-x-clip pt-18 home-wash">
      {hero}
      <div className="mx-auto mt-12 flex w-full max-w-350 flex-col gap-12 px-5 pb-16">
        {children}
      </div>
    </main>
  );
}
