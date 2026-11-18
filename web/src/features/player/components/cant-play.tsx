import type { LinkOptions } from "@tanstack/react-router";

import { BackLink } from "@/features/player/components/back-link";

/** Says the Version can't play here: why, as the reasons that decided its plan say, or else in general. */
export function CantPlay({ back, reasons }: { back: LinkOptions; reasons: string[] }) {
  return (
    <main className="mx-auto flex min-h-dvh max-w-md flex-col justify-center px-6">
      <h1 className="text-2xl font-semibold tracking-tight text-balance">
        This Version can’t play here
      </h1>
      {reasons.length > 0 ? (
        <ul className="mt-2 space-y-1 text-muted-foreground">
          {reasons.map((reason) => (
            <li key={reason}>{reason}</li>
          ))}
        </ul>
      ) : (
        <p className="mt-2 text-muted-foreground">
          This browser can’t play it. Another browser, or another Version, may play.
        </p>
      )}
      <div className="-ms-2.5 mt-6">
        <BackLink back={back} variant="ghost">
          Back
        </BackLink>
      </div>
    </main>
  );
}
