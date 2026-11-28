import type { TitleCandidate } from "@/gen/nama/v1/match_pb";
import { withYear } from "@/shared/lib/unmatched-names";

/** A title found, named with its year, and under it its original title if that differs. */
export function CandidateName({ candidate }: { candidate: TitleCandidate }) {
  const { originalTitle } = candidate;
  return (
    <span className="flex min-w-0 flex-col">
      <span className="text-base">{withYear(candidate.title, candidate.year)}</span>
      {originalTitle !== undefined && originalTitle !== candidate.title && (
        <span className="text-xs font-normal text-pretty">{originalTitle}</span>
      )}
    </span>
  );
}
