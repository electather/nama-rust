import { useRouteContext } from "@tanstack/react-router";

import { RequestActions } from "@/components/request-actions";
import { RequestProgress, RequestStateLabel } from "@/components/request-state";
import type {
  FollowedShow,
  QualityChange,
  Request,
  RequestedQuality,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { dayOf, metadataLine, seasonName, yearOf } from "@/lib/browse";
import { may } from "@/lib/permissions";

/** A Show's Followed Shows and Season Requests, listed together under it. */
export interface ShowGroup {
  showTmdbId: bigint;
  showTitle: string;
  followed: FollowedShow[];
  seasons: Request[];
}

/** A Show's title, with its Followed Shows and Season Requests listed under it. */
export function ShowRequests({ showTitle, followed, seasons }: Omit<ShowGroup, "showTmdbId">) {
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
  if (request.$typeName === "nama.v1.FollowedShow") return "Followed Show";
  const { movie, season } = request;
  return season ? seasonName(season.seasonNumber) : movie?.title;
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
  if (shown === undefined) return shown;
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
function releasedOf({ season, movie }: Request) {
  return season?.airDate ?? movie?.releaseDate;
}

/** Why it was Declined, or why a Request Failed, in the decliner's or the Fulfiller's words. */
function reasonOf(request: Request | FollowedShow) {
  return request.declineReason ?? ("failureReason" in request ? request.failureReason : undefined);
}

function Reason({ reason }: { reason: string | undefined }) {
  return (
    reason !== undefined && (
      <p className="text-sm text-pretty wrap-break-word text-muted-foreground">Reason: {reason}</p>
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
