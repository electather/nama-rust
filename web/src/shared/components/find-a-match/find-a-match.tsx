import { type ReactNode, useId } from "react";

import { MatchFooter } from "@/shared/components/find-a-match/match-footer";
import { SearchFields } from "@/shared/components/find-a-match/search-fields";
import { SearchResults } from "@/shared/components/find-a-match/search-results";
import { useFindAMatch } from "@/shared/hooks/use-find-a-match";
import type { ToMatch } from "@/shared/lib/find-a-match";
import { titleWord } from "@/shared/lib/title-word";

/**
 * Searches the Library's title sources for `item`'s title as the Title and Year are typed, shows
 * the titles found to pick one of, and Matches `item` to it with one Match. What `above` holds
 * scrolls with the page, the search stays at the top of a tall one, and `cancel` sits beside Match, which stays at the foot of a tall page.
 */
export function FindAMatch({
  above,
  cancel,
  ...toMatch
}: ToMatch & { above?: ReactNode; cancel?: ReactNode }) {
  const headingId = useId();
  const { item } = toMatch;
  const { typed, setTyped, found, picked, setChosen, matching, notice, refusal, match } =
    useFindAMatch(toMatch);
  // Sticks at the top as a long list of results scrolls, so the search is always in reach.
  const search = (
    <div className="sticky top-0 z-1 -my-2 flex flex-col gap-3.5 bg-inherit py-2">
      <h4 id={headingId} className="text-sm font-medium">
        Which {titleWord(item.kind)} is it?
      </h4>
      <SearchFields typed={typed} onType={setTyped} />
    </div>
  );
  return (
    <form
      className="flex min-h-full flex-1 flex-col gap-6 bg-inherit"
      onSubmit={(event) => {
        event.preventDefault();
        match();
      }}
    >
      {above}
      <section aria-labelledby={headingId} className="flex flex-col gap-3.5 bg-inherit">
        {search}
        <SearchResults found={found} picked={picked} onPick={setChosen} />
      </section>
      <MatchFooter
        item={item}
        cancel={cancel}
        disabled={picked === undefined || matching}
        notice={notice}
        refusal={refusal}
      />
    </form>
  );
}
