import { createQueryOptions, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { LibraryService, type PreviewDeleteMediaResponse } from "@/gen/nama/v1/library_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";
import { type Deletable, targetOf } from "@/shared/lib/deletable";
import { refusalOf } from "@/shared/lib/forms";

/**
 * Deletes `chosen` of `deletable`, naming the Episodes its preview showed taken along. If a scan
 * changed what its files hold since, the Server refuses it, and the preview loads again to show it.
 */
export function useDeleteMedia(deletable: Deletable) {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const remove = useSensitiveMutation(LibraryService.method.deleteMedia);
  return async (chosen: string) => {
    const target = targetOf(deletable, chosen);
    const preview = LibraryService.method.previewDeleteMedia;
    const { queryKey } = createQueryOptions(preview, { target }, { transport });
    try {
      return await remove.mutateAsync({
        target,
        also: shownIds(queryClient.getQueryData(queryKey)),
      });
    } catch (error) {
      if (refusalOf(error).reason === RefusalReason.TAKES_ALONG_UNNAMED) {
        await queryClient.invalidateQueries({ queryKey });
      }
      throw error;
    }
  };
}

/** The ids of the Episodes `shown` takes along. */
function shownIds(shown: PreviewDeleteMediaResponse | undefined) {
  return (shown?.episodesTakenAlong ?? []).flatMap(({ episodeId }) => episodeId ?? []);
}
