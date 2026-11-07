import type { ReactNode } from "react";

/** A grid of posters, or what to say instead when there are none. */
export function PosterGrid({ empty, children }: { empty: ReactNode; children: ReactNode[] }) {
  if (children.length === 0) return empty;
  return (
    <ul className="mt-8 grid grid-cols-2 gap-x-4 gap-y-8 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
      {children}
    </ul>
  );
}

/**
 * A title's poster with its name and a line of metadata under it. The 2:3 box is there before the
 * image loads, so the grid doesn't shift.
 */
export function Poster({
  url,
  title,
  detail,
}: {
  url?: string | undefined;
  title: string;
  detail?: string | undefined;
}) {
  return (
    <>
      <div className="aspect-2/3 overflow-hidden rounded-lg border bg-muted">
        {url !== undefined && (
          // Its title is the caption, so the image itself says nothing more.
          <img src={url} alt="" loading="lazy" className="size-full object-cover" />
        )}
      </div>
      <p className="mt-2 text-sm font-medium text-balance">{title}</p>
      {detail !== undefined && <p className="text-sm text-muted-foreground">{detail}</p>}
    </>
  );
}
