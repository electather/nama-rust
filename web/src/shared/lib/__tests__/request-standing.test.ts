import { create, fromJson } from "@bufbuild/protobuf";
import { afterEach, beforeEach, describe, expect, test, vi } from "vite-plus/test";

import { RequestSchema, RequestState } from "@/gen/nama/v1/request_pb";
import { type TitleFacts, titleStanding } from "@/shared/lib/request-standing";

const you = 1n;
const asked = (state: RequestState, by = 2n, downloads: { percent: number }[] = []) =>
  create(RequestSchema, { id: 5n, state, requesters: [{ userId: by }], downloads });
/** A Request in a state this app doesn't know, as only a Server newer than it sends. */
const fromANewerServer = fromJson(RequestSchema, { state: 99, requesters: [{ userId: "2" }] });

/** The standing's kind and what you can do, with the Season's beneath one not aired yet. */
function read(facts: TitleFacts) {
  const standing = titleStanding({ you, ...facts });
  return standing.kind === "notAired"
    ? `notAired ${standing.airDate} (${standing.season.kind}): ${standing.can}`
    : `${standing.kind}: ${standing.can}`;
}

describe("where a missing title stands with its Request", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-11-27T12:00:00"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  test.each<{ case: string; facts: TitleFacts; reads: string }>([
    {
      case: "nobody asked opens a Request",
      facts: { request: undefined, inLibrary: false },
      reads: "nobodyAsked: open",
    },
    {
      case: "a Declined Request reads as none",
      facts: { request: asked(RequestState.DECLINED, you), inLibrary: false },
      reads: "nobodyAsked: open",
    },
    {
      case: "an Available Request reads as none",
      facts: { request: asked(RequestState.AVAILABLE), inLibrary: false },
      reads: "nobodyAsked: open",
    },
    {
      case: "in a Library offers nothing, whatever its Request",
      facts: { request: asked(RequestState.PENDING), inLibrary: true },
      reads: "inLibrary: nothing",
    },
    {
      case: "someone else's Pending Request is joined",
      facts: { request: asked(RequestState.PENDING), inLibrary: false },
      reads: "onItsWay: join",
    },
    {
      case: "someone else's Approved Request is joined",
      facts: { request: asked(RequestState.APPROVED), inLibrary: false },
      reads: "onItsWay: join",
    },
    {
      case: "your own Pending Request offers nothing",
      facts: { request: asked(RequestState.PENDING, you), inLibrary: false },
      reads: "onItsWay: nothing",
    },
    {
      case: "Processing without downloads is on its way",
      facts: { request: asked(RequestState.PROCESSING), inLibrary: false },
      reads: "onItsWay: join",
    },
    {
      case: "Processing with downloads is downloading, and joined",
      facts: { request: asked(RequestState.PROCESSING, 2n, [{ percent: 40 }]), inLibrary: false },
      reads: "downloading: join",
    },
    {
      case: "your own download offers nothing",
      facts: { request: asked(RequestState.PROCESSING, you, [{ percent: 40 }]), inLibrary: false },
      reads: "downloading: nothing",
    },
    {
      case: "Failed is stalled, and can't be joined",
      facts: { request: asked(RequestState.FAILED), inLibrary: false },
      reads: "stalled: nothing",
    },
    {
      case: "Partially Available offers nothing to ask or join",
      facts: { request: asked(RequestState.PARTIALLY_AVAILABLE), inLibrary: false },
      reads: "partlyArrived: nothing",
    },
    {
      case: "a newer Server's state is unknown",
      facts: { request: fromANewerServer, inLibrary: false },
      reads: "unknown: nothing",
    },
    {
      case: "only a Followed Show asking reads as followed",
      facts: { request: undefined, inLibrary: false, followed: true },
      reads: "followedOnly: nothing",
    },
    {
      case: "a Followed Show with an open Season Request reads as that Request",
      facts: { request: asked(RequestState.APPROVED), inLibrary: false, followed: true },
      reads: "onItsWay: join",
    },
    {
      case: "an Episode not aired yet says when, over its Season's standing",
      facts: { request: undefined, inLibrary: false, airDate: "2026-12-03" },
      reads: "notAired 2026-12-03 (nobodyAsked): open",
    },
    {
      case: "an Episode not aired yet in a Season in a Library offers nothing",
      facts: { request: undefined, inLibrary: true, airDate: "2026-12-03" },
      reads: "notAired 2026-12-03 (inLibrary): nothing",
    },
    {
      case: "an Episode airing today has aired",
      facts: { request: undefined, inLibrary: false, airDate: "2026-11-27" },
      reads: "nobodyAsked: open",
    },
    {
      case: "an Episode aired before today has aired",
      facts: { request: asked(RequestState.PENDING), inLibrary: false, airDate: "2026-11-26" },
      reads: "onItsWay: join",
    },
  ])("$case", ({ facts, reads }) => {
    expect(read(facts)).toBe(reads);
  });
});
