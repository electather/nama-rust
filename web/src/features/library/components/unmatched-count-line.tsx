import { useUnmatched } from "@/features/library/data/use-unmatched";
import { countLine } from "@/features/library/lib/count-line";

const names = { one: "Not matched", many: "Not matched" };

/** "3 Not matched": what a Library's header says while its grid lists only its Unmatched Items. */
export function UnmatchedCountLine({ libraryId }: { libraryId: bigint }) {
  const { totalSize } = useUnmatched(libraryId);
  return countLine({ shown: totalSize, all: totalSize }, names);
}
