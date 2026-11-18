import { useQuery as useConnectQuery, useTransport } from "@connectrpc/connect-query";
import { useQuery } from "@tanstack/react-query";
import { useRef, useState } from "react";

import type { Quality } from "@/gen/nama/v1/plugin_pb";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { signedInQuery } from "@/lib/access";
import { may } from "@/shared/lib/permissions";

/**
 * The Quality picker for Requesting a Movie or a Show's Seasons, as `kind` says, which `open`
 * opens when the signed-in User is `opening` a Request, may choose a Quality, and has more than
 * one to choose from: joining a Request keeps its Quality, as a Show's Seasons keep the Show's.
 * `open` says whether it opened. `loading` while the Qualities are read, so a Request isn't sent
 * before the User could pick.
 */
export function useQualityPicker(opening: boolean, kind: "movie" | "show") {
  const mayChoose = useMayChooseQuality(opening);
  const listed = useListedQualities(kind, mayChoose);
  const ref = useRef<HTMLDialogElement>(null);
  const [openings, setOpenings] = useState(0);
  const qualities = toPick(mayChoose, listed.data?.qualities);
  const picks = qualities.length > 0;
  const personalDefault = listed.data?.personalDefault;
  return {
    loading: listed.isLoading,
    hasPopup: picks && ("dialog" as const),
    open: () => {
      if (picks) {
        setOpenings((count) => count + 1);
        ref.current?.showModal();
      }
      return picks;
    },
    dialog: { ref, openings, qualities, personalDefault },
  };
}

/** The Qualities of the Fulfiller for `kind`, read only while `enabled`. */
function useListedQualities(kind: "movie" | "show", enabled: boolean) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the Request button waits on `loading` rather than the page, and nothing is read for a User who can't choose
  const movies = useConnectQuery(
    RequestService.method.listMovieQualities,
    {},
    { enabled: enabled && kind === "movie" },
  );
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the Request button waits on `loading` rather than the page, and nothing is read for a User who can't choose
  const shows = useConnectQuery(
    RequestService.method.listShowQualities,
    {},
    { enabled: enabled && kind === "show" },
  );
  return kind === "movie" ? movies : shows;
}

/**
 * The `listed` Qualities, if the User may choose and there's more than one, or none. Checks
 * `mayChoose` even with `listed`, since a disabled query still gives what's cached.
 */
function toPick(mayChoose: boolean, listed: Quality[] = []) {
  return mayChoose && listed.length > 1 ? listed : [];
}

/** Whether the signed-in User may choose a Quality when `opening` a Request. */
function useMayChooseQuality(opening: boolean) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the signed-in User is read for a Permission only, and the picker is just left out until it's known
  const you = useQuery({ ...signedInQuery(useTransport()), staleTime: "static" }).data;
  return opening && may(you?.user, Permission.CHOOSE_QUALITY);
}
