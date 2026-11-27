import { ServerImg } from "@/shared/components/server-img";
import type { ServerImage } from "@/shared/lib/server-image";

/**
 * What the page shows where the video was while it plays in a window of its own: the title's
 * `art`, dimmed, saying so. Shown by CSS alone, since `:picture-in-picture` says when.
 */
export function PictureInPictureCover({ art }: { art: ServerImage | undefined }) {
  return (
    <div className="pointer-events-none absolute inset-0 hidden items-center justify-center bg-background group-has-picture-in-picture:flex">
      {/* Dimmed as one, so its thumbhash doesn't show through at full strength. */}
      <div className="absolute inset-0 isolate opacity-25">
        <ServerImg
          images={[art]}
          sizes="100vw"
          // The first frame's cover, the same copy, so it's there once the window opens: hidden, a
          // lazy image never loads.
          largest
          className="size-full object-cover"
        />
      </div>
      <p className="relative text-sm text-muted-foreground">Playing in picture in picture</p>
    </div>
  );
}
