import type { ReactNode } from "react";

/** A page's main column, under its heading and the `art` beside it, if any. */
export function Page({
  heading,
  art,
  children,
}: {
  heading: string;
  art?: ReactNode;
  children: ReactNode;
}) {
  return (
    <main className="mx-auto w-full max-w-5xl px-4 py-10 sm:px-6">
      <div className="flex items-center gap-6">
        {art}
        <h1 className="text-2xl font-semibold tracking-tight text-balance">{heading}</h1>
      </div>
      {children}
    </main>
  );
}
