/** A Featured title's Content Rating, in its box, then its year, length and genres. */
export function FeaturedFacts({ rating, facts }: { rating: string | undefined; facts: string }) {
  return (
    <div className="flex flex-wrap items-center gap-2.5 text-sm">
      {rating !== undefined && (
        <span className="inline-flex h-5 items-center rounded-sm border border-input px-1.5 text-xs">
          {rating}
        </span>
      )}
      <span className="tabular-nums">{facts}</span>
    </div>
  );
}
