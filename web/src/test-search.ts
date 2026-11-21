import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ServiceImpl } from "@connectrpc/connect";

import { RequestService } from "@/gen/nama/v1/request_pb";
import { type HitSchema, type SearchResponseSchema, SearchService } from "@/gen/nama/v1/search_pb";
import { fakeServer, newUser } from "@/test-app";

type Part = MessageInitShape<typeof SearchResponseSchema>;

/**
 * A fake `Search` that streams each of `batches`, read anew for every search so it sees what a
 * test changed since, then `Done`, flagging the title source unavailable if `unavailable`.
 */
export function searchFinding(
  batches: () => MessageInitShape<typeof HitSchema>[][],
  unavailable = false,
): ServiceImpl<typeof SearchService>["search"] {
  return () =>
    new ReadableStream<Part>({
      start(controller) {
        for (const hits of batches()) {
          controller.enqueue({ part: { case: "hits", value: { hits } } });
        }
        controller.enqueue({
          part: { case: "done", value: { titleSourceUnavailable: unavailable } },
        });
        controller.close();
      },
    });
}

/**
 * A fake Server for a new User, with no Requests, whose `Search` streams `batches` as
 * `searchFinding` does.
 */
export function searchServer(batches: MessageInitShape<typeof HitSchema>[][], unavailable = false) {
  return fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(SearchService, { search: searchFinding(() => batches, unavailable) });
    router.service(RequestService, { listRequests: () => ({}) });
  });
}
