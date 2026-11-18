import type { TitleCandidate } from "@/gen/nama/v1/match_pb";
import { CandidateRow } from "@/shared/components/find-a-match/candidate-row";
import { useSetMatch } from "@/shared/hooks/use-set-match";
import { notInShow, setMatchRefusal } from "@/shared/lib/find-a-match";
import type { ItemToMatch } from "@/shared/lib/unmatched-names";

/** The titles found, each with a button that Matches `item` to it, and what Matching did. */
export function Candidates({
  candidates,
  item,
  libraryId,
}: {
  candidates: TitleCandidate[];
  item: ItemToMatch;
  libraryId: bigint;
}) {
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
              setMatch.mutate({ itemId: item.itemId, titleId: candidate.id });
            }}
          />
        ))}
      </ul>
      <output className="block text-sm text-pretty empty:hidden">
        {setMatch.data && notInShow(item, setMatch.data)}
      </output>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {setMatch.error && setMatchRefusal(setMatch.error)}
      </p>
    </>
  );
}
