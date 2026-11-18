import type { ReactNode } from "react";

/** The `heading` over the `backdrop`, and the scrim that keeps it legible over any image. */
export function TitleHero({
  art,
  backdrop,
  heading,
}: {
  art: boolean;
  backdrop: ReactNode;
  heading: ReactNode;
}) {
  return (
    <div className="relative isolate">
      {backdrop}
      <div
        className="group flex min-h-40 flex-col justify-end pt-10 data-art:min-h-72 sm:data-art:min-h-112"
        data-art={art ? "" : undefined}
      >
        {/* The art is clear above the title, so it shows, and the title sits on at least the
            scrim's 70% black however many lines it takes, fading into the page at the bottom. */}
        <div className="hidden h-24 bg-linear-to-t from-scrim group-data-art:block" />
        {heading}
      </div>
    </div>
  );
}
