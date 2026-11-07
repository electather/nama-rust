import type { ReactNode } from "react";

/** A page's main content under its heading. */
export function Page({ heading, children }: { heading: string; children: ReactNode }) {
  return (
    <main className="mx-auto w-full max-w-5xl px-4 py-10 sm:px-6">
      <h1 className="text-2xl font-semibold tracking-tight text-balance">{heading}</h1>
      {children}
    </main>
  );
}
