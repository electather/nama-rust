import type { TitleCandidate } from "@/gen/nama/v1/match_pb";
import { withYear } from "@/shared/lib/unmatched-names";
import { Button } from "@/shared/ui/button";

/** One title found, with the button that Matches to it. */
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
