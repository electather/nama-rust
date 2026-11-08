import { useMutation } from "@connectrpc/connect-query";
import { Library, Plus } from "lucide-react";
import { useId } from "react";

import {
  RequestStateLabel,
  askRefusal,
  isRequester,
  useReadBackAsked,
} from "@/components/request-state";
import { StateLabel } from "@/components/state-label";
import { Button } from "@/components/ui/button";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type FoundSeason, type FoundShow, RequestService } from "@/gen/nama/v1/request_pb";
import { seasonName, yearOf } from "@/lib/browse";
import { onSubmitted } from "@/lib/forms";

/**
 * `show`'s Seasons, each with where it is on its way to the Server, to pick and Request in one go.
 * The signed-in User `you` may pick a Season that's neither in a Library nor already theirs:
 * picking one someone else Requested joins its Request.
 */
export function SeasonPicker({ show, you }: { show: FoundShow; you: bigint | undefined }) {
  const ask = useMutation(RequestService.method.createSeasonRequests, useReadBackAsked());
  const pickable = (season: FoundSeason) => !season.inLibrary && !isRequester(season.request, you);
  return (
    <form
      className="mt-10 max-w-xl"
      onSubmit={onSubmitted((picked, form) => {
        const seasonNumbers = picked.getAll("season").map(Number);
        ask.mutate(
          { showTmdbId: show.tmdbId, seasonNumbers },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      })}
    >
      <fieldset>
        <legend className="font-medium">Seasons</legend>
        {show.seasons.length === 0 && (
          <p className="mt-3 text-muted-foreground">{show.title} has no Seasons yet.</p>
        )}
        <ul className="mt-3 flex flex-col">
          {show.seasons.map((season) => (
            <SeasonRow key={season.seasonNumber} season={season} pickable={pickable(season)} />
          ))}
        </ul>
      </fieldset>
      {show.seasons.some(pickable) && (
        <div className="mt-6">
          <Button
            type="submit"
            aria-label={`Request Seasons of ${show.title}`}
            disabled={ask.isPending}
          >
            <Plus aria-hidden className="size-4" />
            Request
          </Button>
        </div>
      )}
      {ask.isError && (
        <p role="alert" className="mt-3 text-sm text-destructive">
          {seasonsRefusal(ask.error, show.title)}
        </p>
      )}
    </form>
  );
}

function seasonsRefusal(error: unknown, title: string) {
  return askRefusal(error, `Request Seasons of ${title}`, {
    [RefusalReason.FIELD_REQUIRED]: { form: "Pick at least one Season." },
    // A scan Matched one since the page was opened, which reading the Seasons back shows.
    [RefusalReason.ALREADY_IN_LIBRARY]: { form: "One of those Seasons is already in a Library." },
  });
}

/**
 * A Season as a checkbox, with the year it aired and its state at the end of the row. The state
 * describes the checkbox, so a screen reader says why one can't be picked.
 */
function SeasonRow({ season, pickable }: { season: FoundSeason; pickable: boolean }) {
  const stateId = useId();
  return (
    <li className="flex min-h-12 flex-wrap items-center gap-x-3 gap-y-1 border-b py-2 last:border-b-0">
      <label className="flex items-center gap-3 font-medium has-disabled:text-muted-foreground">
        <input
          type="checkbox"
          name="season"
          value={season.seasonNumber}
          disabled={!pickable}
          aria-describedby={stateId}
          className="size-4 accent-foreground"
        />
        {seasonName(season.seasonNumber)}
      </label>
      <span className="text-sm text-muted-foreground">{yearOf(season.airDate)}</span>
      <span id={stateId} className="ms-auto">
        <SeasonState season={season} />
      </span>
    </li>
  );
}

function SeasonState({ season }: { season: FoundSeason }) {
  if (season.inLibrary) return <StateLabel label="In a Library" Icon={Library} />;
  return season.request && <RequestStateLabel state={season.request.state} />;
}
