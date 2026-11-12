import { type ReactNode, useCallback, useState } from "react";

import { Thumbhash } from "@/components/thumbhash";
import { Button } from "@/components/ui/button";
import { metadataLine } from "@/lib/browse";
import { useSrcSet } from "@/lib/image-sizes";

/**
 * A Movie's or Show's page: its Title Logo, or else its title as text, over its backdrop, then
 * `children`. The backdrop sits under the scrim, so the title keeps its contrast whatever the
 * image (DESIGN.md, "The Legible Over Art Rule"). Only the title goes over the art: Quiet ink
 * wouldn't keep 4.5:1 there. Its `backdropThumbhash` shows until the backdrop loads.
 */
export function TitlePage({
  title,
  logoUrl,
  backdropUrl,
  backdropThumbhash,
  children,
}: {
  title: string;
  logoUrl?: string | undefined;
  backdropUrl: string | undefined;
  backdropThumbhash: Uint8Array | undefined;
  children: ReactNode;
}) {
  const srcSet = useSrcSet(backdropUrl);
  return (
    <main>
      <div className="relative isolate">
        {backdropUrl !== undefined && (
          <>
            <Thumbhash thumbhash={backdropThumbhash} />
            <img
              src={backdropUrl}
              srcSet={srcSet}
              sizes="100vw"
              alt=""
              className="absolute inset-0 -z-10 size-full object-cover"
            />
            {/* Clear at the top, so the art shows, and from 60% up from the bottom at least the
                scrim's 70% black, fading into the page: the title sits there, at the bottom. */}
            <div className="absolute inset-0 -z-10 bg-linear-to-t from-background via-scrim via-60% to-transparent" />
          </>
        )}
        <div
          className="mx-auto flex min-h-40 max-w-5xl items-end px-4 pt-10 pb-6 data-backdrop:min-h-72 sm:px-6 sm:data-backdrop:min-h-112"
          data-backdrop={backdropUrl === undefined ? undefined : ""}
        >
          <h1 className="text-display font-medium text-balance">
            {logoUrl === undefined ? title : <TitleLogo url={logoUrl} title={title} />}
          </h1>
        </div>
      </div>
      <div className="mx-auto max-w-5xl px-4 pb-16 sm:px-6">{children}</div>
    </main>
  );
}

/** A Title Logo, named by `title` so the heading it stands in says it to screen readers. */
function TitleLogo({ url, title }: { url: string; title: string }) {
  const srcSet = useSrcSet(url);
  return (
    <img
      src={url}
      srcSet={srcSet}
      sizes="(min-width: 640px) 448px, 80vw"
      alt={title}
      className="max-h-28 w-auto max-w-4/5 object-contain sm:max-h-36 sm:max-w-md"
    />
  );
}

/**
 * A title's facts in one line, such as "Movie · Drama · 2019", skipping unknowns, then each of
 * `boxed` in a box of its own, such as its Content Rating and "4K".
 */
export function TitleFacts({
  facts,
  boxed,
}: {
  facts: (string | undefined)[];
  boxed: (string | undefined)[];
}) {
  return (
    <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-muted-foreground">
      <span>{metadataLine(...facts)}</span>
      {boxed.filter(Boolean).map((fact) => (
        <span
          key={fact}
          className="rounded-sm border border-input px-1.5 text-xs leading-5 font-medium"
        >
          {fact}
        </span>
      ))}
    </p>
  );
}

/** A title's overview in two lines, with "More" to show the rest, when it doesn't fit. */
export function Overview({ text }: { text: string }) {
  const [clamped, setClamped] = useState(false);
  const [open, setOpen] = useState(false);
  const measure = useCallback(
    (paragraph: HTMLParagraphElement | null) => {
      const update = () => {
        // Open, nothing is clamped, but "Less" must stay.
        if (paragraph && !open) setClamped(paragraph.scrollHeight > paragraph.clientHeight);
      };
      update();
      // Again when its width changes, and once the web font loads, which changes how many lines
      // the text takes but not the clamped box's size.
      const observer = new ResizeObserver(update);
      if (paragraph) observer.observe(paragraph);
      // happy-dom, which the tests run on, has no `document.fonts`.
      if ("fonts" in document) void document.fonts.ready.then(update);
      return () => {
        observer.disconnect();
      };
    },
    [open],
  );
  return (
    <div className="mt-6 max-w-prose">
      <p
        ref={measure}
        className="text-pretty data-clamped:line-clamp-2"
        data-clamped={open ? undefined : ""}
      >
        {text}
      </p>
      {clamped && (
        // Pulled out by the button's own padding, so its text lines up with the overview's. It
        // stays once open, so keyboard focus isn't dropped.
        <div className="-ms-4">
          <Button
            variant="ghost"
            aria-expanded={open}
            onClick={() => {
              setOpen(!open);
            }}
          >
            {open ? "Less" : "More"}
          </Button>
        </div>
      )}
    </div>
  );
}
