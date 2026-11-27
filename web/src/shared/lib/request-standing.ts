import { type Request, RequestState } from "@/gen/nama/v1/request_pb";
import { isLaterDay } from "@/shared/lib/title-facts";

/** What the signed-in User can do about a missing title: open a Request, join one, or nothing. */
export type Can = "open" | "join" | "nothing";

/**
 * Where a missing title stands with its Request, and what the signed-in User `can` do. `onItsWay`
 * is Pending, Approved or Processing, `downloading` Processing with downloads, `stalled` Failed
 * till a Request manager retries it, `partlyArrived` Partially Available, `unknown` a newer
 * Server's state. An Episode `notAired` yet stands as its `season` does but for its own words.
 */
export type Standing = { can: Can } & (
  | { kind: "inLibrary" | "nobodyAsked" | "followedOnly" }
  | { kind: "notAired"; airDate: string; season: Standing }
  | { kind: "onItsWay" | "downloading" | "stalled" | "partlyArrived" | "unknown"; request: Request }
);

/** A `Standing` with an open Request, which a Request pill draws. */
export type RequestStanding = Extract<Standing, { request: Request }>;

/**
 * What a title's standing is worked out from: its open Request the User may see, if any; whether a
 * Library has it or, for a Season or Episode, its Season; the signed-in User `you`; an Episode's
 * `airDate` and `now`, the clock's by default; and, for the command menu, whether a Followed Show
 * asks for it.
 */
export interface TitleFacts {
  request: Request | undefined;
  inLibrary: boolean;
  you?: bigint | undefined;
  airDate?: string | undefined;
  now?: number;
  followed?: boolean;
}

/** Where the title `facts` tells of stands, and what the signed-in User can do about it. */
export function titleStanding(facts: TitleFacts): Standing {
  const standing = asIfAired(facts);
  const { airDate, now } = facts;
  return airDate !== undefined && isLaterDay(airDate, now)
    ? { kind: "notAired", airDate, season: standing, can: standing.can }
    : standing;
}

/** What a `Standing` that may be an Episode's says of its Season. */
export function seasonStanding(standing: Standing) {
  return standing.kind === "notAired" ? standing.season : standing;
}

const kinds: Partial<Record<RequestState, RequestStanding["kind"]>> = {
  [RequestState.PENDING]: "onItsWay",
  [RequestState.APPROVED]: "onItsWay",
  [RequestState.PROCESSING]: "onItsWay",
  [RequestState.FAILED]: "stalled",
  [RequestState.PARTIALLY_AVAILABLE]: "partlyArrived",
};

/** A Declined or Available Request is closed, as `Request` in `request.proto` says. */
const closed = new Set<RequestState>([RequestState.DECLINED, RequestState.AVAILABLE]);

/** The standing of `facts`' title, or an Episode's Season, its air date aside. */
function asIfAired({ request, inLibrary, you, followed = false }: TitleFacts): Standing {
  if (inLibrary) {
    return { kind: "inLibrary", can: "nothing" };
  }
  if (request === undefined || closed.has(request.state)) {
    return followed
      ? { kind: "followedOnly", can: "nothing" }
      : { kind: "nobodyAsked", can: "open" };
  }
  const kind = requestKind(request);
  const joinable = kind === "onItsWay" || kind === "downloading";
  const asker = request.requesters.some(({ userId }) => userId === you);
  return { kind, request, can: joinable && !asker ? "join" : "nothing" };
}

function requestKind({ state, downloads }: Request): RequestStanding["kind"] {
  if (state === RequestState.PROCESSING && downloads.length > 0) {
    return "downloading";
  }
  return kinds[state] ?? "unknown";
}
