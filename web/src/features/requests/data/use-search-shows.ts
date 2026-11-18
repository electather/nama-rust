import { useQuery } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";

/** The Shows the title source found for `title`. */
export function useSearchShows(title: string) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- a search shows "Searching…" and its own error under the page's form, which stays up
  return useQuery(RequestService.method.searchShows, { title });
}
