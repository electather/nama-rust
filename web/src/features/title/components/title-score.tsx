import { Star } from "lucide-react";

/** A title's Score, out of 10, as a star and one decimal, such as ★ 8.4. */
export function TitleScore({ score }: { score: number }) {
  return (
    <span className="flex items-center gap-1 tabular-nums">
      <Star aria-hidden className="size-3 fill-current" />
      <span className="sr-only">Score</span>
      {score.toFixed(1)}
    </span>
  );
}
