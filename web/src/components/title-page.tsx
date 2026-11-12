import type { ReactNode } from "react";

import { useSrcSet } from "@/lib/image-sizes";

/**
 * A Movie's or Show's page: its title over its backdrop, then `children`. The backdrop sits under
 * the scrim, so the title keeps its contrast whatever the image (DESIGN.md, "The Legible Over Art
 * Rule"). Only the title goes over the art: Quiet ink wouldn't keep 4.5:1 there.
 */
export function TitlePage({
  title,
  backdropUrl,
  children,
}: {
  title: string;
  backdropUrl: string | undefined;
  children: ReactNode;
}) {
  const srcSet = useSrcSet(backdropUrl);
  return (
    <main>
      <div className="relative isolate">
        {backdropUrl !== undefined && (
          <>
            <img
              src={backdropUrl}
              srcSet={srcSet}
              sizes="100vw"
              alt=""
              className="absolute inset-0 -z-10 size-full object-cover"
            />
            {/* At least the scrim's 70% black everywhere, fading into the page at the bottom. */}
            <div className="absolute inset-0 -z-10 bg-linear-to-t from-background to-scrim" />
          </>
        )}
        <div
          className="mx-auto flex min-h-40 max-w-5xl items-end px-4 pt-10 pb-6 data-backdrop:min-h-72 sm:px-6 sm:data-backdrop:min-h-96"
          data-backdrop={backdropUrl === undefined ? undefined : ""}
        >
          <h1 className="text-display font-medium text-balance">{title}</h1>
        </div>
      </div>
      <div className="mx-auto max-w-5xl px-4 pb-16 sm:px-6">{children}</div>
    </main>
  );
}
