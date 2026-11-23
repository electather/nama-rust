import type { ReactNode } from "react";

/**
 * The card a signed-out page, or signing in another Device, holds its one task in, under its
 * `heading` when it has one throughout.
 */
export function SignInCard({
  heading,
  intro,
  children,
}: {
  heading?: string | undefined;
  /** What the page is for, under its heading, where the form alone doesn't say. */
  intro?: ReactNode;
  children: ReactNode;
}) {
  return (
    <main className="flex w-full max-w-110 flex-col gap-5 rounded-xl border bg-card p-7.5">
      {heading !== undefined && (
        <h1 className="text-center text-3xl font-medium tracking-tight text-balance">{heading}</h1>
      )}
      {intro !== undefined && (
        <p className="-mt-2 text-center text-sm text-pretty text-muted-foreground">{intro}</p>
      )}
      {children}
    </main>
  );
}
