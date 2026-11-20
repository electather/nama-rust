import { SeasonFieldset } from "@/features/title/components/season-fieldset";
import { ShowQuality } from "@/features/title/components/show-quality";
import { useAskForShow } from "@/features/title/data/use-ask-for-show";
import { seasonsRefusal } from "@/features/title/lib/seasons-refusal";
import type { FoundSeason, FoundShow } from "@/gen/nama/v1/request_pb";
import { RequestSplitButton } from "@/shared/components/request-button/request-split-button";
import { canAsk } from "@/shared/lib/can-ask";
import { isRequester } from "@/shared/lib/is-requester";

/**
 * `show`'s Seasons, each with where it is on its way to the Server, to pick and Request in one go,
 * or the Whole Show, which covers the numbered ones and later ones too. The signed-in User `you`
 * may pick a Season that's not in a Library and that they can ask for, or the Whole Show unless
 * they follow it: picking one someone else Requested joins its Request. Seasons download at the Show's
 * Quality once it has one; until then, a User who may choose can pick it from Request's menu.
 */
export function SeasonPicker({ show, you }: { show: FoundShow; you: bigint | undefined }) {
  const { whole, setWhole, asking, formRef, askAt } = useAskForShow(show);
  const pickable = (season: FoundSeason) => !season.inLibrary && canAsk(season.request, you);
  const wholePickable = !isRequester(show.followedShow, you);
  return (
    <form ref={formRef} className="max-w-xl">
      <SeasonFieldset
        show={show}
        wholePickable={wholePickable}
        pickable={(season) => !whole && pickable(season)}
        onWholeChange={setWhole}
      />
      <ShowQuality show={show} />
      {(wholePickable || show.seasons.some(pickable)) && (
        <div className="mt-6">
          <RequestSplitButton
            what={`Seasons of ${show.title}`}
            kind="show"
            opening={show.quality === undefined}
            primary
            busy={asking.isPending}
            onRequest={askAt}
          />
        </div>
      )}
      {asking.isError && (
        <p role="alert" className="mt-3 text-sm text-destructive">
          {seasonsRefusal(asking.error, show.title)}
        </p>
      )}
    </form>
  );
}
