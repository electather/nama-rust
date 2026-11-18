import { type QualityTarget, qualityTargetOf } from "@/features/requests/lib/quality-target";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type FollowedShow,
  type Request,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { refusal, showHasQuality } from "@/shared/lib/forms";
import { seasonName } from "@/shared/lib/title-facts";

/** The RPCs that change a Request by its ID, what it's called, and what cancelling one does. */
const requestChanges = {
  noun: "Request",
  approve: RequestService.method.approveRequest,
  decline: RequestService.method.declineRequest,
  cancel: RequestService.method.cancelRequest,
  withdraw: RequestService.method.withdrawRequest,
  cancelling: "It’s removed for everyone who asked for it. This can’t be undone.",
};

/** The RPCs that change a Followed Show by its ID, what it's called, and what cancelling one does. */
const followedShowChanges = {
  noun: "Followed Show",
  approve: RequestService.method.approveFollowedShow,
  decline: RequestService.method.declineFollowedShow,
  cancel: RequestService.method.cancelFollowedShow,
  withdraw: RequestService.method.withdrawFollowedShow,
  cancelling:
    "It’s removed for everyone who asked for it, and later Seasons won’t be Requested. Its Season Requests stay. This can’t be undone.",
};

/** The Request or Followed Show a change is to, with how to word and make it. */
export interface Target {
  id: bigint;
  /** What the Request asks for, or the Followed Show's Show: "the Request for" it names it. */
  title: string;
  /** What names it to the User on its own: a Request's `title`, or "the Followed Show for" it. */
  label: string;
  /**
   * Both kinds' RPCs take an `id`, and Decline's a `reason` too, but a union of the two can't be
   * inferred, so the components call them as taking any message.
   */
  changes: typeof requestChanges | typeof followedShowChanges;
  /** Its Quality, which approving it, or retrying a Request, can change. */
  quality?: QualityTarget | undefined;
}

/** `request` as a {@link Target}. */
export function targetOf(request: Request | FollowedShow): Target {
  const { id } = request;
  if (request.$typeName === "nama.v1.FollowedShow") {
    return {
      id,
      title: request.showTitle,
      label: `the Followed Show for ${request.showTitle}`,
      changes: followedShowChanges,
      quality: qualityTargetOf(request),
    };
  }
  const quality = qualityTargetOf(request);
  return { id, title: nameOf(request), label: nameOf(request), changes: requestChanges, quality };
}

/** What `request` asks for, in words: a Movie's title, or a Show's with the Season's name. */
function nameOf({ movieOrSeason }: Request) {
  if (movieOrSeason.case === "season") {
    return `${movieOrSeason.value.showTitle} ${seasonName(movieOrSeason.value.seasonNumber)}`;
  }
  return movieOrSeason.value?.title ?? "this Movie";
}

/**
 * Whether the User `you` is one of `request`'s Requesters, and it's Pending or a Followed Show:
 * withdrawing from an Approved one stops following the Show.
 */
export function canWithdraw(request: Request | FollowedShow, you: bigint | undefined) {
  const open =
    request.$typeName === "nama.v1.FollowedShow" || request.state === RequestState.PENDING;
  return open && request.requesters.some(({ userId }) => userId === you);
}

/**
 * Why the Server refused to `change` the Request or Followed Show `target`, such as another User
 * changing it first.
 */
export function changeRefusal(
  error: unknown,
  { title, label, changes: { noun } }: Omit<Target, "id">,
  change: string,
) {
  // A Followed Show has reasons of its own, which say the same to its Requesters.
  const decided = { form: `${upperFirst(label)} was already approved or declined.` };
  const gone = { form: `The ${noun} for ${title} is gone.` };
  const noLongerRequester = { form: `You’re no longer a Requester of ${label}.` };
  return refusal(
    error,
    `${change} this ${noun}`,
    {},
    {
      [RefusalReason.REQUEST_NOT_PENDING]: decided,
      [RefusalReason.REQUEST_NOT_FOUND]: gone,
      [RefusalReason.NOT_A_REQUESTER]: noLongerRequester,
      [RefusalReason.REQUEST_NOT_FAILED]: { form: `${label} isn’t Failed any more.` },
      [RefusalReason.QUALITY_NOT_OFFERED]: { form: "That Quality isn’t offered any more." },
      [RefusalReason.SHOW_HAS_QUALITY]: { form: showHasQuality(error, "this Show") },
      [RefusalReason.FOLLOWED_SHOW_NOT_PENDING]: decided,
      [RefusalReason.FOLLOWED_SHOW_NOT_FOUND]: gone,
      [RefusalReason.NOT_A_FOLLOWED_SHOW_REQUESTER]: noLongerRequester,
    },
  ).form;
}

function upperFirst(text: string) {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
