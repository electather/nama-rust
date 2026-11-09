import { Library, Plus } from "lucide-react";
import { type ReactNode, useId } from "react";

import { seasonsRefusal, useAskForShow } from "@/components/ask-for-show";
import { QualityPicker } from "@/components/quality-picker";
import { qualityName } from "@/components/request-row";
import { RequestProgress, RequestStateLabel, isRequester } from "@/components/request-state";
import { StateLabel } from "@/components/state-label";
import { Button } from "@/components/ui/button";
import type { FollowedShow, FoundSeason, FoundShow } from "@/gen/nama/v1/request_pb";
import { seasonName, yearOf } from "@/lib/browse";

/**
 * `show`'s Seasons, each with where it is on its way to the Server, to pick and Request in one go,
 * or the Whole Show, which covers them and later ones too. The signed-in User `you` may pick a
 * Season that's neither in a Library nor already theirs, or the Whole Show unless they follow it:
 * picking one someone else Requested joins its Request. Seasons download at the Show's Quality
 * once it has one; until then, Requesting them or the Whole Show asks which, when the User may
 * choose.
 */
export function SeasonPicker({ show, you }: { show: FoundShow; you: bigint | undefined }) {
  const { whole, setWhole, asking, busy, submit, picker } = useAskForShow(show);
  const pickable = (season: FoundSeason) => !season.inLibrary && !isRequester(season.request, you);
  const wholePickable = !isRequester(show.followedShow, you);
  return (
    <>
      <form className="mt-10 max-w-xl" onSubmit={submit}>
        <fieldset>
          <legend className="font-medium">Seasons</legend>
          <ul className="mt-3 flex flex-col">
            <WholeShowChoice
              followedShow={show.followedShow}
              disabled={!wholePickable}
              onChange={setWhole}
            />
            <SeasonChoices show={show} pickable={(season) => !whole && pickable(season)} />
          </ul>
        </fieldset>
        <ShowQuality show={show} />
        {(wholePickable || show.seasons.some(pickable)) && (
          <RequestButton title={show.title} busy={busy} hasPopup={picker.hasPopup} />
        )}
        {asking.isError && (
          <p role="alert" className="mt-3 text-sm text-destructive">
            {seasonsRefusal(asking.error, show.title)}
          </p>
        )}
      </form>
      {/* Outside the form above, since the dialog holds a form of its own. */}
      <QualityPicker
        {...picker.dialog}
        title={askedFor(show.title, whole)}
        onPick={picker.onPick}
        refusal={(error) => seasonsRefusal(error, show.title)}
      />
    </>
  );
}

/** What Requesting the Show titled `title` asks for, the Whole Show or Seasons, in words. */
function askedFor(title: string, whole: boolean) {
  return whole ? `the Whole Show of ${title}` : `Seasons of ${title}`;
}

/** The Quality `show`'s Seasons download at, once it has one. */
function ShowQuality({ show }: { show: FoundShow }) {
  return (
    show.quality && (
      <p className="mt-3 text-sm text-muted-foreground">
        Every Season downloads at {qualityName(show.quality)}, the Show’s Quality.
      </p>
    )
  );
}

/** The form's submit button, which Requests what's picked of the Show titled `title`. */
function RequestButton({
  title,
  busy,
  hasPopup,
}: {
  title: string;
  busy: boolean;
  hasPopup: false | "dialog";
}) {
  return (
    <div className="mt-6">
      <Button
        type="submit"
        aria-label={`Request Seasons of ${title}`}
        aria-haspopup={hasPopup}
        disabled={busy}
      >
        <Plus aria-hidden className="size-4" />
        Request
      </Button>
    </div>
  );
}

/** The Whole Show as a Choice, with the state of its open Followed Show, if it has one. */
function WholeShowChoice({
  followedShow,
  ...choice
}: { followedShow: FollowedShow | undefined } & Pick<ChoiceProps, "disabled" | "onChange">) {
  return (
    <Choice
      name="whole"
      label="Whole Show"
      description="Every Season, and new ones as they come out"
      {...choice}
    >
      {followedShow && <RequestStateLabel state={followedShow.state} />}
    </Choice>
  );
}

/** Each of `show`'s Seasons as a Choice, which can be picked if it's `pickable`. */
function SeasonChoices({
  show,
  pickable,
}: {
  show: FoundShow;
  pickable: (season: FoundSeason) => boolean;
}) {
  if (show.seasons.length === 0) {
    return <li className="py-2 text-muted-foreground">{show.title} has no Seasons yet.</li>;
  }
  return show.seasons.map((season) => (
    <Choice
      key={season.seasonNumber}
      name="season"
      value={season.seasonNumber}
      label={seasonName(season.seasonNumber)}
      detail={yearOf(season.airDate)}
      disabled={!pickable(season)}
      below={<RequestProgress request={season.request} />}
    >
      <SeasonState season={season} />
    </Choice>
  ));
}

interface ChoiceProps {
  /** The form field the checkbox fills, with `value` when checked. */
  name: string;
  value?: number;
  label: string;
  /** What the row says after the label, such as the year a Season aired. */
  detail?: string | undefined;
  /** What the choice covers, on a line of its own under the label. */
  description?: string;
  disabled: boolean;
  onChange?: (checked: boolean) => void;
  /** What follows its state, such as a Season's downloads, on lines of their own. */
  below?: ReactNode;
  /** Its state, at the end of the row. */
  children: ReactNode;
}

/**
 * One choice as a checkbox, with its detail and its state at the end of the row. The state
 * describes the checkbox, so a screen reader says why one can't be picked.
 */
function Choice({
  name,
  value,
  label,
  detail,
  description,
  disabled,
  onChange,
  below,
  children,
}: ChoiceProps) {
  const stateId = useId();
  const descriptionId = useId();
  return (
    <li className="flex min-h-12 flex-wrap items-center gap-x-3 gap-y-1 border-b py-2 last:border-b-0">
      <label className="flex items-center gap-3 font-medium has-disabled:text-muted-foreground">
        <input
          type="checkbox"
          name={name}
          value={value}
          disabled={disabled}
          aria-describedby={description === undefined ? stateId : `${descriptionId} ${stateId}`}
          className="size-4 accent-foreground"
          onChange={
            onChange &&
            ((event) => {
              onChange(event.currentTarget.checked);
            })
          }
        />
        {label}
      </label>
      <span className="text-sm text-muted-foreground">{detail}</span>
      <span id={stateId} className="ms-auto">
        {children}
      </span>
      {description !== undefined && (
        // Lined up with the label's text, past the checkbox and the gap after it.
        <p id={descriptionId} className="basis-full ps-7 text-sm text-muted-foreground">
          {description}
        </p>
      )}
      {/* Hidden when empty, so a row with nothing below gains no gap. */}
      <div className="basis-full ps-7 empty:hidden">{below}</div>
    </li>
  );
}

function SeasonState({ season }: { season: FoundSeason }) {
  if (season.inLibrary) return <StateLabel label="In a Library" Icon={Library} />;
  return season.request && <RequestStateLabel state={season.request.state} />;
}
