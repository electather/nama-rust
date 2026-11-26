import { type Download, type Request, RequestState } from "@/gen/nama/v1/request_pb";

/**
 * What a missing Episode's corner shows for its Season's `request`: + to ask while it has none, or
 * one Declined, a ring while it's downloading, or else its state.
 */
export type CornerShown =
  | { kind: "ask" }
  | { kind: "ring"; downloads: Download[] }
  | { kind: "state"; state: RequestState };

/** The `CornerShown` for `request`. */
export function cornerShown(request: Request | undefined): CornerShown {
  if (request === undefined || request.state === RequestState.DECLINED) {
    return { kind: "ask" };
  }
  return downloading(request)
    ? { kind: "ring", downloads: request.downloads }
    : { kind: "state", state: request.state };
}

/** Whether `request` is Processing with downloads to show how far they've got. */
function downloading(request: Request) {
  return request.state === RequestState.PROCESSING && request.downloads.length > 0;
}

/** Whether asking for a Season with `request` opens a Request, whose Quality may then be picked. */
export function opensRequest(request: Request | undefined) {
  return request === undefined || request.state === RequestState.DECLINED;
}
