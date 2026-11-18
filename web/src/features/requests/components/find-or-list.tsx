import { FindATitle } from "@/features/requests/components/find-a-title";
import { RequestList } from "@/features/requests/components/request-list";
import { TitlesFound } from "@/features/requests/components/titles-found";
import type { Searchable } from "@/features/requests/lib/searchable";
import type { StateFilter } from "@/features/requests/lib/state-filter";

/** The search, then what it found for `title`, or the Requests before anything is searched for. */
export function FindOrList({
  title,
  kinds,
  you,
  filter,
  choose,
}: {
  title: string | undefined;
  kinds: Searchable;
  you: bigint | undefined;
  filter: StateFilter;
  choose: (state: StateFilter) => void;
}) {
  return (
    <>
      <FindATitle title={title} kinds={kinds} />
      {title === undefined ? (
        <RequestList filter={filter} choose={choose} />
      ) : (
        <TitlesFound title={title} kinds={kinds} you={you} />
      )}
    </>
  );
}
