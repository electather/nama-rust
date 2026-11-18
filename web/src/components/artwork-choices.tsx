import { cn } from "cn";
import { useId } from "react";

import { useSrcSet } from "@/lib/image-sizes";

/** Artwork to pick as an image: its URLs, best first, and the shape to show them in. */
export interface Choices {
  urls: string[];
  shape: keyof typeof choiceShapes;
}

/** Each shape's artwork to pick from, smaller than the preview, and the width it's shown at. */
const choiceShapes = {
  poster: { className: "w-12 aspect-2/3", sizes: "48px" },
  backdrop: { className: "w-24 aspect-video", sizes: "96px" },
};

/**
 * The artwork the title sources list for the image `name`, each a button that picks it; nothing
 * if they list none.
 */
export function ArtworkChoices({
  name,
  choices: { urls, shape },
  pending,
  onPicked,
}: {
  name: string;
  choices: Choices;
  pending: boolean;
  onPicked: (url: string) => void;
}) {
  const id = useId();
  if (urls.length === 0) {
    return null;
  }
  return (
    <>
      <p id={id} className="mt-2 text-sm text-muted-foreground">
        Or pick one from the title sources
      </p>
      {/* Scrolls sideways, so a long list keeps the dialog short. */}
      <ul aria-labelledby={id} className="mt-2 flex gap-2 overflow-x-auto p-1">
        {urls.map((url, index) => (
          <li key={url} className="shrink-0">
            <button
              type="button"
              aria-label={`Pick ${name} ${String(index + 1)} of ${String(urls.length)}`}
              disabled={pending}
              onClick={() => {
                onPicked(url);
              }}
              className={cn(
                "block overflow-hidden rounded-md border bg-muted outline-none hover:ring-2 hover:ring-foreground/40 focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none",
                choiceShapes[shape].className,
              )}
            >
              <ChoiceImage url={url} sizes={choiceShapes[shape].sizes} />
            </button>
          </li>
        ))}
      </ul>
    </>
  );
}

/** One artwork to pick, at the smallest copy the Server resizes to that fits `sizes`. */
function ChoiceImage({ url, sizes }: { url: string; sizes: string }) {
  const srcSet = useSrcSet(url);
  // The button names it, so the image itself says nothing more.
  return (
    <img
      src={url}
      srcSet={srcSet}
      sizes={sizes}
      alt=""
      loading="lazy"
      className="size-full object-cover"
    />
  );
}
