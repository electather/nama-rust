import { SeasonFieldset } from "@/features/title/components/season-fieldset";
import { SeasonRequestButton } from "@/features/title/components/season-request-button";
import { ShowQuality } from "@/features/title/components/show-quality";
import { useAskForShow } from "@/features/title/data/use-ask-for-show";
import { askedFor, seasonsRefusal } from "@/features/title/lib/seasons-refusal";
import type { FoundSeason, FoundShow } from "@/gen/nama/v1/request_pb";
import { QualityPicker } from "@/shared/components/quality-picker/quality-picker";
import { isRequester } from "@/shared/lib/is-requester";

/**
 * `show`'s Seasons, each with where it is on its way to the Server, to pick and Request in one go,
 * or the Whole Show, which covers the numbered ones and later ones too. The signed-in User `you`
 * may pick a Season that's neither in a Library nor already theirs, or the Whole Show unless they
 * follow it: picking one someone else Requested joins its Request. Seasons download at the Show's
 * Quality once it has one; until then, Requesting them or the Whole Show asks which, when the User
 * may choose.
 */
export function SeasonPicker({ show, you }: { show: FoundShow; you: bigint | undefined }) {
  const { whole, setWhole, asking, busy, submit, picker } = useAskForShow(show);
  const { onPick, dialog, hasPopup } = picker;
  const pickable = (season: FoundSeason) => !season.inLibrary && !isRequester(season.request, you);
  const wholePickable = !isRequester(show.followedShow, you);
  return (
    <>
      <form className="mt-10 max-w-xl" onSubmit={submit}>
        <SeasonFieldset
          show={show}
          wholePickable={wholePickable}
          pickable={(season) => !whole && pickable(season)}
          onWholeChange={setWhole}
        />
        <ShowQuality show={show} />
        {(wholePickable || show.seasons.some(pickable)) && (
          <SeasonRequestButton title={show.title} busy={busy} hasPopup={hasPopup} />
        )}
        {asking.isError && (
          <p role="alert" className="mt-3 text-sm text-destructive">
            {seasonsRefusal(asking.error, show.title)}
          </p>
        )}
      </form>
      {/* Outside the form above, since the dialog holds a form of its own. */}
      <QualityPicker
        {...dialog}
        title={askedFor(show.title, whole)}
        onPick={onPick}
        refusal={(error) => seasonsRefusal(error, show.title)}
      />
    </>
  );
}
