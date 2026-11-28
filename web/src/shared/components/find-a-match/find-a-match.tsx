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
 * scrolls with the page, and `cancel` sits beside Match, which stays at the foot of a tall page.
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
  return (
    <form
      className="flex min-h-full flex-1 flex-col gap-6 bg-inherit"
      onSubmit={(event) => {
        event.preventDefault();
        match();
      }}
    >
      {above}
      <section aria-labelledby={headingId} className="flex flex-col gap-3.5">
        <h4 id={headingId} className="text-base font-medium">
          Which {titleWord(item.kind)} is it?
        </h4>
        <SearchFields typed={typed} onType={setTyped} />
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
