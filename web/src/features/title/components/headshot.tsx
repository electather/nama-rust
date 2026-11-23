import { UserRound } from "lucide-react";

import { useSrcSet } from "@/shared/hooks/use-src-set";

/** A Person's round headshot, 128px, or a plain circle with a person in it. */
export function Headshot({ url }: { url: string | undefined }) {
  const srcSet = useSrcSet(url);
  if (url === undefined) {
    return (
      <span className="flex size-32 shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground">
        <UserRound aria-hidden className="size-12" />
      </span>
    );
  }
  // Its name is beside it, so the image itself says nothing more.
  return (
    <img
      src={url}
      srcSet={srcSet}
      sizes="128px"
      alt=""
      loading="lazy"
      className="size-32 shrink-0 rounded-full bg-muted object-cover outline -outline-offset-1 outline-input"
    />
  );
}
