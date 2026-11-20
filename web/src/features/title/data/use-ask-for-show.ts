import { useMutation } from "@connectrpc/connect-query";
import { useRef, useState } from "react";

import { type FoundShow, RequestService } from "@/gen/nama/v1/request_pb";
import { useReadBackAsked } from "@/shared/hooks/use-read-back-asked";

/**
 * Asking for the Seasons of `show` picked in the form on `formRef`, or for the Whole Show while
 * `whole`, at the User's Quality or the one `askAt` is given.
 */
export function useAskForShow(show: FoundShow) {
  const readBack = useReadBackAsked();
  const ask = useMutation(RequestService.method.createSeasonRequests, readBack);
  const follow = useMutation(RequestService.method.createFollowedShow, readBack);
  const [whole, setWhole] = useState(false);
  const formRef = useRef<HTMLFormElement>(null);
  const showId = show.id;
  const askAt = (quality?: bigint) => {
    const form = formRef.current;
    if (!form) {
      return;
    }
    const onSuccess = () => {
      form.reset();
      setWhole(false);
    };
    if (whole) {
      follow.mutate({ showId, quality }, { onSuccess });
    } else {
      const seasonNumbers = new FormData(form).getAll("season").map(Number);
      ask.mutate({ showId, seasonNumbers, quality }, { onSuccess });
    }
  };
  return { whole, setWhole, asking: whole ? follow : ask, formRef, askAt };
}
