import { Button } from "nama-ui/components/button";

import type { TitleCandidate } from "@/gen/nama/v1/match_pb";
import { ServerImg } from "@/shared/components/server-img";
import { posterOf } from "@/shared/lib/server-image";
import { withYear } from "@/shared/lib/unmatched-names";

const posterBox =
  "aspect-2/3 h-15 shrink-0 rounded-xs bg-muted outline -outline-offset-1 outline-border";

/** One title found, with its Poster and the button that Matches to it. */
export function CandidateRow({
  candidate,
  pending,
  onMatch,
}: {
  candidate: TitleCandidate;
  pending: boolean;
  onMatch: () => void;
}) {
  const name = withYear(candidate.title, candidate.year);
  const { originalTitle } = candidate;
  return (
    <li className="flex items-center gap-3 rounded-lg px-3 py-1 hover:bg-muted">
      {/* The row names the title, so its Poster says nothing more; without one, its box stays. */}
      <ServerImg
        images={[posterOf(candidate)]}
        sizes="40px"
        className={`${posterBox} object-cover`}
        fallback={<span className={posterBox} />}
      />
      <span className="min-w-0 flex-1">
        {name}
        {originalTitle !== undefined && originalTitle !== candidate.title && (
          <span className="ms-2 text-sm text-muted-foreground">{originalTitle}</span>
        )}
      </span>
      <Button variant="ghost" aria-label={`Match to ${name}`} disabled={pending} onClick={onMatch}>
        Match
      </Button>
    </li>
  );
}
