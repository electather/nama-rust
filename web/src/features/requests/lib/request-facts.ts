import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type FollowedShow,
  type QualityChange,
  type Request,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { qualityName } from "@/shared/lib/quality-name";
import { dayOf, metadataLine, seasonName, yearOf } from "@/shared/lib/title-facts";

/**
 * A row's title: a Request's Movie's, or its Season's name, or "Followed Show", either of which
 * its Show's title is above.
 */
export function titleOf(request: Request | FollowedShow) {
  if (request.$typeName === "nama.v1.FollowedShow") {
    return "Followed Show";
  }
  const { movieOrSeason } = request;
  return movieOrSeason.case === "season"
    ? seasonName(movieOrSeason.value.seasonNumber)
    : movieOrSeason.value?.title;
}

/**
 * What the row says under its title: the year a Request's title came out, its or a Followed
 * Show's Quality, and who asked.
 */
export function aboutRequest(request: Request | FollowedShow, you: bigint | undefined) {
  if (request.$typeName === "nama.v1.FollowedShow") {
    const { quality } = request;
    return metadataLine(quality && qualityName(quality), requestedBy(request, you));
  }
  return metadataLine(yearOf(releasedOf(request)), qualityOf(request), requestedBy(request, you));
}

/**
 * The Quality a Request is downloaded at: the one its service used once handed off, else the one
 * asked for, a profile no Quality offers being the service's own. Then who last changed it from
 * what, and when: "HD, changed from 4K by Sam on 9 Nov 2026".
 */
function qualityOf({ quality, qualityUsed, qualityChange }: Request) {
  const shown = qualityUsed ?? quality;
  if (shown === undefined) {
    return shown;
  }
  return qualityChange ? `${qualityName(shown)}, ${changeText(qualityChange)}` : qualityName(shown);
}

/** Who changed a Quality from what, and when, in words: "changed from 4K by Sam on 9 Nov 2026". */
function changeText({ previousName, changedBy = "a Request manager", changedAt }: QualityChange) {
  const from = previousName === undefined ? "" : ` from ${previousName}`;
  return `changed${from} by ${changedBy} on ${dayOf(changedAt)}`;
}

/** When a Request's Movie came out, or its Season first aired. */
function releasedOf({ movieOrSeason }: Request) {
  return movieOrSeason.case === "season"
    ? movieOrSeason.value.airDate
    : movieOrSeason.value?.releaseDate;
}

/**
 * Why it was Declined, or why a Request Failed: in the decliner's or the Fulfiller's words, or in
 * the app's when the Server didn't hand it to the Fulfiller for lack of an id.
 */
export function reasonOf(request: Request | FollowedShow) {
  if (request.$typeName === "nama.v1.FollowedShow") {
    return request.declineReason;
  }
  return request.declineReason ?? missingIdText(request) ?? request.failureReason;
}

// "en-GB" for "TVDB or IMDb", without the serial comma, as the rest of the app writes lists.
const idList = new Intl.ListFormat("en-GB", { type: "disjunction" });

/**
 * "Sonarr can’t fetch it: no TVDB id is known for this Show.", when the Server failed `request`
 * because its Fulfiller needs an id of the title that it lacks.
 */
function missingIdText({ failure, movieOrSeason }: Request) {
  const kind = movieOrSeason.case === "season" ? "Show" : "Movie";
  return failure?.reason === RefusalReason.TITLE_ID_MISSING
    ? missingIdSentence(failure.params, kind)
    : undefined;
}

function missingIdSentence(
  { pluginId = "", namespaces = "" }: Record<string, string>,
  kind: "Movie" | "Show",
) {
  // ponytail: the Plugin's id capitalised stands in for its name, which the Request doesn't carry.
  const plugin =
    pluginId === "" ? "Its Fulfiller" : pluginId.charAt(0).toUpperCase() + pluginId.slice(1);
  const ids = idList.format(namespaces.split(",").map((namespace) => namespace.toUpperCase()));
  return `${plugin} can’t fetch it: no ${ids} id is known for this ${kind}.`;
}

/** The Plugin whose Fulfiller didn't take a Failed Request, if it names one. */
export function failedBy(request: Request | FollowedShow) {
  return request.$typeName === "nama.v1.Request" && request.state === RequestState.FAILED
    ? request.fulfillerPluginId
    : undefined;
}

// "en-GB" for "Ada, Grace and you", without the serial comma, as the rest of the app writes lists.
const names = new Intl.ListFormat("en-GB", { type: "conjunction" });

/** Who asked, first asker first, with the signed-in User `you` as "you". */
function requestedBy({ requesters }: Pick<Request, "requesters">, you: bigint | undefined) {
  const who = requesters.map(({ userId, name }) => (userId === you ? "you" : name));
  return `Requested by ${names.format(who)}`;
}

/** What a Request's Fulfiller is downloading for it; a Followed Show downloads nothing itself. */
export function downloadsOf(request: Request | FollowedShow) {
  return request.$typeName === "nama.v1.Request" ? request.downloads : [];
}
