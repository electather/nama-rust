import type { TitleCandidate } from "@/gen/nama/v1/match_pb";
import { CandidateRow } from "@/shared/components/find-a-match/candidate-row";
import { useSetMatch } from "@/shared/hooks/use-set-match";
import { type ToMatch, matchTarget, notInShow, setMatchRefusal } from "@/shared/lib/find-a-match";

/** The titles found, each with a button that Matches `item` to it, and what Matching did. */
export function Candidates({
  candidates,
  item,
  libraryId,
  onMatched,
}: ToMatch & { candidates: TitleCandidate[] }) {
  const setMatch = useSetMatch(libraryId);
  return (
    <>
      <ul aria-label="Titles found" className="-mx-3 flex flex-col">
        {candidates.map((candidate) => (
          <CandidateRow
            key={`${candidate.id?.namespace}:${candidate.id?.value}`}
            candidate={candidate}
            pending={setMatch.isPending}
            onMatch={() => {
              setMatch.mutate(
                { ...matchTarget(item), titleId: candidate.id },
                {
                  onSuccess: (matched) => {
                    onMatched?.(candidate.id, matched);
                  },
                },
              );
            }}
          />
        ))}
      </ul>
      <output className="block text-sm text-pretty empty:hidden">
        {setMatch.data && "itemId" in item && notInShow(item, setMatch.data)}
      </output>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {setMatch.error && setMatchRefusal(setMatch.error)}
      </p>
    </>
  );
}
