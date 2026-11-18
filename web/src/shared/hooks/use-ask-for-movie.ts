import { useMutation } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";
import { useReadBackAsked } from "@/shared/hooks/use-read-back-asked";

/** Asks for a Movie found in the title source, then reads the Requests back. */
export function useAskForMovie() {
  return useMutation(RequestService.method.createRequest, useReadBackAsked());
}
