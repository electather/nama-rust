import type { RefObject } from "react";

import { ImageChoices } from "@/features/title/components/image-choices";
import type { TitleImages } from "@/features/title/lib/title-images";
import { Button } from "@/shared/ui/button";

/** The Edit images dialog of the title `title`, which asks for each image's artwork anew each `opened`. */
export function EditImagesDialog({
  ref,
  id,
  title,
  images,
  opened,
}: TitleImages & {
  ref: RefObject<HTMLDialogElement | null>;
  id: string;
  title: string;
  opened: number;
}) {
  // ponytail: the native modal <dialog> traps focus, closes on Escape and returns focus itself.
  return (
    <dialog
      ref={ref}
      aria-labelledby={`${id}-title`}
      aria-describedby={`${id}-description`}
      // Floats above the page, so it alone gets a shadow (DESIGN.md, "Elevation").
      className="m-auto w-full max-w-sm rounded-xl border bg-popover p-6 text-popover-foreground shadow-float backdrop:bg-scrim sm:max-w-lg"
    >
      <h2 id={`${id}-title`} className="text-section text-balance">
        Images of {title}
      </h2>
      <p id={`${id}-description`} className="mt-2 text-sm text-pretty text-muted-foreground">
        Everyone sees the images you upload or pick in place of the downloaded ones, until you
        remove them. Uploads are JPEG, PNG or WebP, up to 20 MB.
      </p>
      {opened > 0 && <ImageChoices key={opened} images={images} />}
      <div className="mt-6 flex justify-end">
        <Button variant="ghost" onClick={() => ref.current?.close()}>
          Done
        </Button>
      </div>
    </dialog>
  );
}
