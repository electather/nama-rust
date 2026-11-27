import { ServerImg } from "@/shared/components/server-img";
import type { ServerImage } from "@/shared/lib/server-image";

/**
 * What the page shows where the video was while it plays in a window of its own: the title's
 * `art`, dimmed, saying so. Shown by CSS alone, since `:picture-in-picture` says when.
 */
export function PictureInPictureCover({ art }: { art: ServerImage | undefined }) {
  return (
    <div className="pointer-events-none absolute inset-0 isolate hidden items-center justify-center bg-background group-has-picture-in-picture:flex">
      <ServerImg
        images={[art]}
        sizes="100vw"
        className="absolute inset-0 size-full object-cover opacity-25"
      />
      <p className="relative text-sm text-muted-foreground">Playing in picture in picture</p>
    </div>
  );
}
