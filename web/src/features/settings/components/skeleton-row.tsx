const BONE = "animate-pulse rounded-sm bg-muted motion-reduce:animate-none";

/** One row of a pane that's loading: a box and two lines, with a hairline between rows as `ItemRow` draws it. */
export function SkeletonRow() {
  return (
    <div aria-hidden className="group flex gap-3.5">
      <span className={`size-10 shrink-0 rounded-lg group-not-first:mt-3.5 ${BONE}`} />
      <span className="flex flex-1 flex-col justify-center gap-2 pb-3.5 group-not-first:border-t group-not-first:pt-3.5">
        <span className={`h-3 w-1/3 ${BONE}`} />
        <span className={`h-2.5 w-3/5 opacity-60 ${BONE}`} />
      </span>
    </div>
  );
}
