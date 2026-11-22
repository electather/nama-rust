import type { ReactNode } from "react";

/** Every Hub page: the Hub's name over the page's one card, in the middle of the screen. */
export function HubLayout({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-dvh flex-col items-center justify-center gap-7.5 px-5 py-12">
      <p className="text-lg/6 font-semibold tracking-tight">Nama Hub</p>
      <main className="flex w-full max-w-110 flex-col gap-5 rounded-xl border bg-card p-7.5">
        {children}
      </main>
    </div>
  );
}
