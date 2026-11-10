import { useMutation } from "@connectrpc/connect-query";
import { useState } from "react";

import { useQualityPicker } from "@/components/quality-picker";
import { useReadBackAsked } from "@/components/request-state";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type FoundShow, RequestService } from "@/gen/nama/v1/request_pb";
import { askRefusal, onSubmitted, showHasQuality } from "@/lib/forms";

/**
 * Asking for the Seasons of `show` picked in a form, or for the Whole Show while `whole`. Either,
 * for a Show with no Quality yet, is asked for once the picker, if it opens, gives one.
 */
export function useAskForShow(show: FoundShow) {
  const readBack = useReadBackAsked();
  const ask = useMutation(RequestService.method.createSeasonRequests, readBack);
  const follow = useMutation(RequestService.method.createFollowedShow, readBack);
  const [whole, setWhole] = useState(false);
  const picker = useQualityPicker(show.quality === undefined, "show");
  // The Seasons picked when the picker opened, and how to clear the form once they're asked for.
  const [awaitingQuality, setAwaitingQuality] = useState({
    seasonNumbers: [] as number[],
    onSuccess: () => {},
  });
  const showTmdbId = show.tmdbId;
  const askWithoutPicking = (seasonNumbers: number[], onSuccess: () => void) => {
    if (whole) follow.mutate({ showTmdbId }, { onSuccess });
    else ask.mutate({ showTmdbId, seasonNumbers }, { onSuccess });
  };
  const submit = onSubmitted((picked, form) => {
    const onSuccess = () => {
      form.reset();
      setWhole(false);
    };
    const seasonNumbers = picked.getAll("season").map(Number);
    setAwaitingQuality({ seasonNumbers, onSuccess });
    // With no Season picked, the Server's refusal says to pick one, so there's no Quality to ask.
    const picking = (whole || seasonNumbers.length > 0) && picker.open();
    if (!picking) askWithoutPicking(seasonNumbers, onSuccess);
  });
  const onPick = (quality: bigint) => {
    const { seasonNumbers, onSuccess } = awaitingQuality;
    const asked = whole
      ? follow.mutateAsync({ showTmdbId, quality })
      : ask.mutateAsync({ showTmdbId, seasonNumbers, quality });
    return asked.then(onSuccess);
  };
  const asking = whole ? follow : ask;
  return {
    whole,
    setWhole,
    asking,
    busy: asking.isPending || picker.loading,
    submit,
    picker: { ...picker, onPick },
  };
}

/** What to tell the User when Requesting Seasons of the Show titled `title` was refused with `error`. */
export function seasonsRefusal(error: unknown, title: string) {
  return askRefusal(error, `Request Seasons of ${title}`, {
    [RefusalReason.FIELD_REQUIRED]: { form: "Pick at least one Season." },
    // A scan Matched one since the page was opened, which reading the Seasons back shows.
    [RefusalReason.ALREADY_IN_LIBRARY]: { form: "One of those Seasons is already in a Library." },
    // Removed from the list since the page opened.
    [RefusalReason.QUALITY_NOT_OFFERED]: {
      form: "That Quality isn’t offered any more. Reload the page and pick another.",
    },
    // Another Season gave the Show its Quality since the page opened.
    [RefusalReason.SHOW_HAS_QUALITY]: {
      form: `${showHasQuality(error, title)} Reload the page to ask at it.`,
    },
  });
}
