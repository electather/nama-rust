import type { LinkOptions } from "@tanstack/react-router";

import { BackLink } from "@/features/player/components/back-link";
import { Page } from "@/shared/components/page";

/** What the player shows once an Admin stopped its play, with what they said, if anything. */
export function StoppedByAdmin({ message, back }: { message: string; back: LinkOptions }) {
  return (
    <Page heading="An Admin stopped playback">
      {message !== "" && <p className="mt-2 max-w-prose">{message}</p>}
      <p className="mt-2 max-w-prose text-muted-foreground">You can play it again at any time.</p>
      <div className="mt-6">
        <BackLink back={back} variant="secondary">
          Back
        </BackLink>
      </div>
    </Page>
  );
}
