import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

/** Every Hub page: the Hub's name, linking home, over the page's one card, in the middle of the screen. */
export function HubLayout({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-dvh flex-col items-center justify-center gap-7.5 px-5 py-12">
      {/* The tab's name while no page names it, such as while one loads. */}
      <title>Nama Hub</title>
      <Link to="/" className="text-lg/6 font-semibold tracking-tight">
        Nama Hub
      </Link>
      <main className="flex w-full max-w-110 flex-col gap-5 rounded-xl border bg-card p-7.5">
        {children}
      </main>
    </div>
  );
}
