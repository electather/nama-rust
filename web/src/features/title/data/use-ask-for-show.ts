import { useMutation } from "@connectrpc/connect-query";
import { useState } from "react";

import { type FoundShow, RequestService } from "@/gen/nama/v1/request_pb";
import { useQualityPicker } from "@/shared/hooks/use-quality-picker";
import { useReadBackAsked } from "@/shared/hooks/use-read-back-asked";
import { onSubmitted } from "@/shared/lib/forms";

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
  const [awaitingQuality, setAwaitingQuality] = useState<{
    seasonNumbers: number[];
    onSuccess: () => void;
  }>();
  const showId = show.id;
  const askWithoutPicking = (seasonNumbers: number[], onSuccess: () => void) => {
    if (whole) {
      follow.mutate({ showId }, { onSuccess });
    } else {
      ask.mutate({ showId, seasonNumbers }, { onSuccess });
    }
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
    if (!picking) {
      askWithoutPicking(seasonNumbers, onSuccess);
    }
  });
  const onPick = async (quality: bigint) => {
    if (awaitingQuality === undefined) {
      return;
    }
    const { seasonNumbers, onSuccess } = awaitingQuality;
    await (whole
      ? follow.mutateAsync({ showId, quality })
      : ask.mutateAsync({ showId, seasonNumbers, quality }));
    onSuccess();
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
