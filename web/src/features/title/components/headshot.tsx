import { UserRound } from "lucide-react";

import { ServerImg } from "@/shared/components/server-img";
import type { ServerImage } from "@/shared/lib/server-image";

/** A Person's round headshot, 128px, or a plain circle with a person in it. */
export function Headshot({ headshot }: { headshot: ServerImage | undefined }) {
  // Its name is beside it, so the image itself says nothing more.
  return (
    <ServerImg
      images={[headshot]}
      sizes="128px"
      className="size-32 shrink-0 rounded-full bg-muted object-cover outline -outline-offset-1 outline-input"
      fallback={
        <span className="flex size-32 shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground">
          <UserRound aria-hidden className="size-12" />
        </span>
      }
    />
  );
}
