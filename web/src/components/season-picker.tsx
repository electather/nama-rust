import { useMutation } from "@connectrpc/connect-query";
import { Library, Plus } from "lucide-react";
import { type ReactNode, useId, useState } from "react";

import {
  RequestStateLabel,
  askRefusal,
  isRequester,
  useReadBackAsked,
} from "@/components/request-state";
import { StateLabel } from "@/components/state-label";
import { Button } from "@/components/ui/button";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type FollowedShow,
  type FoundSeason,
  type FoundShow,
  RequestService,
} from "@/gen/nama/v1/request_pb";
import { seasonName, yearOf } from "@/lib/browse";
import { onSubmitted } from "@/lib/forms";

/**
 * `show`'s Seasons, each with where it is on its way to the Server, to pick and Request in one go,
 * or the Whole Show, which covers them and later ones too. The signed-in User `you` may pick a
 * Season that's neither in a Library nor already theirs, or the Whole Show unless they follow it:
 * picking one someone else Requested joins its Request.
 */
export function SeasonPicker({ show, you }: { show: FoundShow; you: bigint | undefined }) {
  const { whole, setWhole, asking, submit } = useAskForShow(show);
  const pickable = (season: FoundSeason) => !season.inLibrary && !isRequester(season.request, you);
  const wholePickable = !isRequester(show.followedShow, you);
  return (
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
      {(wholePickable || show.seasons.some(pickable)) && (
        <RequestButton title={show.title} busy={asking.isPending} />
      )}
      {asking.isError && (
        <p role="alert" className="mt-3 text-sm text-destructive">
          {seasonsRefusal(asking.error, show.title)}
        </p>
      )}
    </form>
  );
}

/** The form's submit button, which Requests what's picked of the Show titled `title`. */
function RequestButton({ title, busy }: { title: string; busy: boolean }) {
  return (
    <div className="mt-6">
      <Button type="submit" aria-label={`Request Seasons of ${title}`} disabled={busy}>
        <Plus aria-hidden className="size-4" />
        Request
      </Button>
    </div>
  );
}

/** Asking for the Seasons of `show` picked in a form, or for the Whole Show while `whole`. */
function useAskForShow(show: FoundShow) {
  const readBack = useReadBackAsked();
  const ask = useMutation(RequestService.method.createSeasonRequests, readBack);
  const follow = useMutation(RequestService.method.createFollowedShow, readBack);
  const [whole, setWhole] = useState(false);
  const submit = onSubmitted((picked, form) => {
    const showTmdbId = show.tmdbId;
    const onSuccess = () => {
      form.reset();
      setWhole(false);
    };
    if (whole) follow.mutate({ showTmdbId }, { onSuccess });
    else
      ask.mutate({ showTmdbId, seasonNumbers: picked.getAll("season").map(Number) }, { onSuccess });
  });
  return { whole, setWhole, asking: whole ? follow : ask, submit };
}

function seasonsRefusal(error: unknown, title: string) {
  return askRefusal(error, `Request Seasons of ${title}`, {
    [RefusalReason.FIELD_REQUIRED]: { form: "Pick at least one Season." },
    // A scan Matched one since the page was opened, which reading the Seasons back shows.
    [RefusalReason.ALREADY_IN_LIBRARY]: { form: "One of those Seasons is already in a Library." },
  });
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
    </li>
  );
}

function SeasonState({ season }: { season: FoundSeason }) {
  if (season.inLibrary) return <StateLabel label="In a Library" Icon={Library} />;
  return season.request && <RequestStateLabel state={season.request.state} />;
}
