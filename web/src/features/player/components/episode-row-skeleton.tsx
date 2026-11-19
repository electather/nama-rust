/** Where an Episode row will be while it loads: its Still's box and two lines of text. */
export function EpisodeRowSkeleton() {
  return (
    <div aria-hidden className="flex items-center gap-3 p-2">
      <span className="aspect-video w-28 flex-none animate-pulse rounded-lg bg-muted motion-reduce:animate-none" />
      <span className="flex flex-1 flex-col gap-2">
        <span className="h-3 w-24 animate-pulse rounded-full bg-muted motion-reduce:animate-none" />
        <span className="h-4 w-36 animate-pulse rounded-full bg-muted motion-reduce:animate-none" />
      </span>
    </div>
  );
}
