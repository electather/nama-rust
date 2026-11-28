import { RadioGroup } from "nama-ui/components/radio-group";
import { useState } from "react";

import type { TitleCandidate } from "@/gen/nama/v1/match_pb";
import { CandidateRow } from "@/shared/components/find-a-match/candidate-row";
import { MoreResults } from "@/shared/components/find-a-match/more-results";
import { candidateKey } from "@/shared/lib/find-a-match";

/** How many titles show before "N more results". */
const BEST_FITS = 3;

/**
 * The titles found, the best fits first as radios of which `picked` is chosen, and the rest behind
 * "N more results".
 */
export function Candidates({
  candidates,
  picked,
  onPick,
}: {
  candidates: TitleCandidate[];
  picked: TitleCandidate | undefined;
  onPick: (key: string) => void;
}) {
  const [all, setAll] = useState(false);
  const shown = all ? candidates : candidates.slice(0, BEST_FITS);
  const more = candidates.length - shown.length;
  return (
    <fieldset className="flex min-w-0 flex-col gap-1.5">
      <legend className="pb-1.5 text-xs text-muted-foreground">Best fits</legend>
      <RadioGroup
        value={picked === undefined ? "" : candidateKey(picked)}
        onValueChange={(key) => {
          onPick(String(key));
        }}
      >
        {shown.map((candidate) => (
          <CandidateRow key={candidateKey(candidate)} candidate={candidate} />
        ))}
      </RadioGroup>
      {more > 0 && (
        <MoreResults
          more={more}
          onShow={() => {
            setAll(true);
          }}
        />
      )}
    </fieldset>
  );
}
