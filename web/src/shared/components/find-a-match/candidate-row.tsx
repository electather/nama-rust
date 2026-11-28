import { RadioRow } from "nama-ui/components/radio-group";

import type { TitleCandidate } from "@/gen/nama/v1/match_pb";
import { CandidateName } from "@/shared/components/find-a-match/candidate-name";
import { PosterArt } from "@/shared/components/poster/poster-art";
import { candidateKey } from "@/shared/lib/find-a-match";
import { posterOf } from "@/shared/lib/server-image";
import { withYear } from "@/shared/lib/unmatched-names";

/**
 * One title found, as the radio that picks it: its Poster, its name with its year, and its
 * original title if that differs, the facts that tell same-named titles apart.
 */
export function CandidateRow({ candidate }: { candidate: TitleCandidate }) {
  const name = withYear(candidate.title, candidate.year);
  return (
    <RadioRow value={candidateKey(candidate)}>
      <span className="flex items-center gap-3.5">
        {/* The row names the title, so its Poster says nothing more; without one, its box stays. */}
        <PosterArt art={posterOf(candidate)} title={name} size="thumb" />
        <CandidateName candidate={candidate} />
      </span>
    </RadioRow>
  );
}
