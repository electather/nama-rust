import { MayDownloadAgain } from "@/shared/components/delete-media/may-download-again";
import type { DeletedSearch } from "@/shared/lib/deleted-search";

/** Says which Fulfillers may download what a delete on the Show's page took again, if it took one. */
export function NotToldNotice({ deleted, notTold }: DeletedSearch) {
  return deleted === undefined ? null : (
    <MayDownloadAgain title={deleted} fulfillers={notTold ?? []} />
  );
}
