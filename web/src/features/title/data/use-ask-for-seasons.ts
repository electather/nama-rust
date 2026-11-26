import { useMutation } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";
import { useReadBackAsked } from "@/shared/hooks/use-read-back-asked";

/** Asking for Seasons of a Show, which reads its Requests back once asked. */
export function useAskForSeasons() {
  return useMutation(RequestService.method.createSeasonRequests, useReadBackAsked());
}
