import type { ReactNode } from "react";

/** A signed-out page's narrow column: its heading, what it's for, then its form. */
export function SignInPage({
  heading,
  intro,
  footer,
  children,
}: {
  heading: string;
  intro: ReactNode;
  /** Another way in, under the page's own, such as signing in with a code instead. */
  footer?: ReactNode;
  children: ReactNode;
}) {
  return (
    <main className="mx-auto flex min-h-dvh w-full max-w-sm flex-col justify-center px-6 py-12">
      <h1 className="text-2xl font-semibold tracking-tight text-balance">{heading}</h1>
      <p className="mt-2 text-muted-foreground">{intro}</p>
      <div className="mt-8">{children}</div>
      <div className="mt-6 flex justify-center empty:hidden">{footer}</div>
    </main>
  );
}
