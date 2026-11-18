import type { Request } from "@/gen/nama/v1/request_pb";

/** Whether the signed-in User `you` is one of the Requesters of `request`, or of a Followed Show. */
export function isRequester(
  request: Pick<Request, "requesters"> | undefined,
  you: bigint | undefined,
) {
  return request?.requesters.some(({ userId }) => userId === you) ?? false;
}
