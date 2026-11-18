import { useSearch } from "@tanstack/react-router";

import { MayDownloadAgain } from "@/shared/components/delete-media/may-download-again";

/** Says which Fulfillers may download what a delete on the Show's page took again, as its URL says. */
export function NotToldNotice() {
  const { deleted, notTold = [] } = useSearch({
    from: "/_member/libraries/$libraryId/shows/$showId/",
  });
  return deleted === undefined ? null : <MayDownloadAgain title={deleted} fulfillers={notTold} />;
}
