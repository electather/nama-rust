import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { WAITING_POLL_MS } from "@/features/settings/lib/server-link";
import { UserService } from "@/gen/nama/v1/user_pb";

/** The query key of where the signed-in User's Link stands, to refetch after starting or Unlinking. */
export const userLinkKey = createConnectQueryKey({
  schema: UserService.method.getUserLink,
  cardinality: "finite",
});

/**
 * Where Linking the signed-in User to an Account stands, or `undefined` while the Server can't
 * Link Users: without the Hub Plugin, or before the Server is Linked. Looked at again while a code
 * waits to be entered.
 */
export function useUserLink() {
  return useSuspenseQuery(
    UserService.method.getUserLink,
    {},
    {
      refetchInterval: (query) =>
        (query.state.data?.link?.code ?? "") === "" ? false : WAITING_POLL_MS,
      // As for the Server's Link: the code is entered in another tab, and the Server learns of it
      // only when asked.
      refetchIntervalInBackground: true,
    },
  ).data.link;
}
