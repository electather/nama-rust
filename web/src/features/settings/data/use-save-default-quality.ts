import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { defaultsKey } from "@/features/settings/data/use-default-qualities";
import { RequestService } from "@/gen/nama/v1/request_pb";

function movieQualitiesKey(transport: Transport) {
  return createConnectQueryKey({
    schema: RequestService.method.listMovieQualities,
    cardinality: "finite",
    transport,
  });
}
function showQualitiesKey(transport: Transport) {
  return createConnectQueryKey({
    schema: RequestService.method.listShowQualities,
    cardinality: "finite",
    transport,
  });
}

/** Saving a default Quality, then reading the defaults and the Quality menus' Qualities again. */
export function useSaveDefaultQuality() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(RequestService.method.setDefaultQuality, {
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: defaultsKey(transport) }),
        queryClient.invalidateQueries({ queryKey: movieQualitiesKey(transport) }),
        queryClient.invalidateQueries({ queryKey: showQualitiesKey(transport) }),
      ]);
    },
  });
}
