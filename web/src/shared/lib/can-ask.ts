import { type Request, RequestState } from "@/gen/nama/v1/request_pb";
import { isRequester } from "@/shared/lib/is-requester";

/** The states in which a Request can be joined: those still on their way to the Fulfiller. */
const joinable = new Set<RequestState>([
  RequestState.PENDING,
  RequestState.APPROVED,
  RequestState.PROCESSING,
]);

/**
 * Whether the signed-in User `you` can ask for a title with `request`: with none, or Declined,
 * which opens a new one, or by joining one they aren't a Requester of yet. Not once Available, or
 * Failed until a Request manager retries it.
 */
export function canAsk(request: Request | undefined, you: bigint | undefined) {
  if (request === undefined || request.state === RequestState.DECLINED) {
    return true;
  }
  return joinable.has(request.state) && !isRequester(request, you);
}
