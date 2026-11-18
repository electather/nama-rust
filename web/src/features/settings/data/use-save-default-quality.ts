import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { defaultsKey } from "@/features/settings/data/use-default-qualities";
import { RequestService } from "@/gen/nama/v1/request_pb";

const movieQualitiesKey = createConnectQueryKey({
  schema: RequestService.method.listMovieQualities,
  cardinality: "finite",
});

/** Saving a default Quality, then reading the defaults and the Request dialog's Qualities again. */
export function useSaveDefaultQuality() {
  const queryClient = useQueryClient();
  return useMutation(RequestService.method.setDefaultQuality, {
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: defaultsKey }),
        queryClient.invalidateQueries({ queryKey: movieQualitiesKey }),
      ]);
    },
  });
}
