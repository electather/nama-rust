import { Link, useRouteContext } from "@tanstack/react-router";

import { RequestActions } from "@/components/request-actions";
import { RequestProgress } from "@/components/request-progress";
import { RequestStateLabel } from "@/components/request-state";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type FollowedShow,
  type QualityChange,
  type Request,
  type RequestedQuality,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { dayOf, metadataLine, seasonName, yearOf } from "@/lib/browse";
import { may } from "@/lib/permissions";

/** A Show's Followed Shows and Season Requests, listed together under it. */
export interface ShowGroup {
  showId?: ExternalId | undefined;
  showTitle: string;
  followed: FollowedShow[];
  seasons: Request[];
}

/** A Show's title, with its Followed Shows and Season Requests listed under it. */
export function ShowRequests({ showTitle, followed, seasons }: Omit<ShowGroup, "showId">) {
  return (
    <li className="border-b pt-3 last:border-b-0">
      <p className="truncate font-medium">{showTitle}</p>
      <ul aria-label={showTitle} className="flex flex-col ps-4">
        {followed.map((followedShow) => (
          <RequestRow key={`followed-${followedShow.id}`} request={followedShow} />
        ))}
        {seasons.map((request) => (
          <RequestRow key={String(request.id)} request={request} />
        ))}
      </ul>
    </li>
  );
}

/** A Request, or a Followed Show, with its state and what the signed-in User may do to it. */
export function RequestRow({ request }: { request: Request | FollowedShow }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return (
    // Wraps so the downloads and the actions get lines of their own, the full width of the row.
    <li className="flex min-h-16 flex-wrap items-center gap-x-3 border-b py-3 last:border-b-0">
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{titleOf(request)}</p>
        <p className="text-sm text-muted-foreground">{aboutRequest(request, signedIn.user?.id)}</p>
        <Reason reason={reasonOf(request)} />
        <FulfillerLink
          pluginId={failedBy(request)}
          mayFix={may(signedIn.user, Permission.MANAGE_SETTINGS)}
        />
      </div>
      <RequestStateLabel state={request.state} />
      {request.$typeName === "nama.v1.Request" && <RequestProgress request={request} />}
      <RequestActions
        request={request}
        you={signedIn.user?.id}
        manage={may(signedIn.user, Permission.MANAGE_REQUESTS)}
      />
    </li>
  );
}

/**
 * A row's title: a Request's Movie's, or its Season's name, or "Followed Show", either of which
 * its Show's title is above.
 */
function titleOf(request: Request | FollowedShow) {
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
function aboutRequest(request: Request | FollowedShow, you: bigint | undefined) {
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

/** A Quality's name, or the service's own profile's, marked so, when no Quality offers it. */
export function qualityName({ id, name }: RequestedQuality) {
  return id === 0n ? `${name} (set in the service)` : name;
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
function reasonOf(request: Request | FollowedShow) {
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

function Reason({ reason }: { reason: string | undefined }) {
  return (
    reason !== undefined && (
      <p className="text-sm text-pretty wrap-break-word text-muted-foreground">Reason: {reason}</p>
    )
  );
}

/** The Plugin whose Fulfiller didn't take a Failed Request, if it names one. */
function failedBy(request: Request | FollowedShow) {
  return request.$typeName === "nama.v1.Request" && request.state === RequestState.FAILED
    ? request.fulfillerPluginId
    : undefined;
}

/**
 * The link to the Plugin `pluginId`, whose settings fix why its Fulfiller didn't take a Request,
 * shown only to a User who `mayFix` them there: one with Manage settings.
 */
function FulfillerLink({ pluginId, mayFix }: { pluginId: string | undefined; mayFix: boolean }) {
  return (
    pluginId !== undefined &&
    mayFix && (
      <Link
        to="/admin/plugins/$pluginId"
        params={{ pluginId }}
        className="rounded-sm text-sm underline decoration-muted-foreground underline-offset-4 outline-none hover:decoration-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        Fix it in the Plugin’s settings
      </Link>
    )
  );
}

// "en-GB" for "Ada, Grace and you", without the serial comma, as the rest of the app writes lists.
const names = new Intl.ListFormat("en-GB", { type: "conjunction" });

/** Who asked, first asker first, with the signed-in User `you` as "you". */
function requestedBy({ requesters }: Pick<Request, "requesters">, you: bigint | undefined) {
  const who = requesters.map(({ userId, name }) => (userId === you ? "you" : name));
  return `Requested by ${names.format(who)}`;
}
