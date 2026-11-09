import type { DescMessage } from "@bufbuild/protobuf";
import { useMutation } from "@connectrpc/connect-query";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { FormTextarea } from "@/components/form-field";
import { useRefetchRequests } from "@/components/request-state";
import { ChangeButton, type QualityTarget, qualityTargetOf } from "@/components/change-button";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type FollowedShow,
  type Request,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { seasonName } from "@/lib/browse";
import { formText, refusal } from "@/lib/forms";

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

/**
 * What the signed-in User `you` may do to `request`, a Request or a Followed Show: withdraw from it
 * if it's their own and Pending, or their own Followed Show, and approve, decline, retry or cancel
 * it if they `manage` Requests.
 */
export function RequestActions({
  request,
  you,
  manage,
}: {
  request: Request | FollowedShow;
  you: bigint | undefined;
  manage: boolean;
}) {
  const target = targetOf(request);
  return (
    // Pulled back by the buttons' own padding, so the first one's words line up with the title.
    // Hidden when it has no buttons, so it adds no space to the row.
    <div className="-ms-4 mt-1 flex basis-full flex-wrap gap-1 empty:hidden">
      {canWithdraw(request, you) && <Withdraw {...target} />}
      {manage && <Manage state={request.state} {...target} />}
    </div>
  );
}

/**
 * Whether the User `you` is one of `request`'s Requesters, and it's Pending or a Followed Show:
 * withdrawing from an Approved one stops following the Show.
 */
function canWithdraw(request: Request | FollowedShow, you: bigint | undefined) {
  const open =
    request.$typeName === "nama.v1.FollowedShow" || request.state === RequestState.PENDING;
  return open && request.requesters.some(({ userId }) => userId === you);
}

/** Approve and Decline for a Pending Request, Retry for a Failed one, and Cancel for any. */
function Manage({ state, ...target }: Target & { state: RequestState }) {
  return (
    <>
      {state === RequestState.PENDING && (
        <>
          <Approve {...target} />
          <Decline {...target} />
        </>
      )}
      {state === RequestState.FAILED && <Retry {...target} />}
      <Cancel {...target} />
    </>
  );
}

interface Target {
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
  /** A Request's Quality, which approving or retrying it can change; a Followed Show has none. */
  quality?: QualityTarget;
}

function targetOf(request: Request | FollowedShow): Target {
  const { id } = request;
  if (request.$typeName === "nama.v1.FollowedShow") {
    return {
      id,
      title: request.showTitle,
      label: `the Followed Show for ${request.showTitle}`,
      changes: followedShowChanges,
    };
  }
  const quality = qualityTargetOf(request);
  return { id, title: nameOf(request), label: nameOf(request), changes: requestChanges, quality };
}

/** What `request` asks for, in words: a Movie's title, or a Show's with the Season's name. */
function nameOf({ movie, season }: Request) {
  if (season) return `${season.showTitle} ${seasonName(season.seasonNumber)}`;
  return movie?.title ?? "this Movie";
}

function Approve({ id, ...target }: Target) {
  const approve = useMutation<DescMessage, DescMessage>(
    target.changes.approve,
    useRefetchRequests(),
  );
  return (
    <ChangeButton
      action="Approve"
      name={`Approve ${target.label}`}
      change={approve}
      id={id}
      refusal={(error) => changeRefusal(error, target, "approve")}
      quality={target.quality}
    />
  );
}

/** Hands a Failed Request to its Fulfiller again. */
function Retry({ id, ...target }: Target) {
  const retry = useMutation(RequestService.method.retryRequest, useRefetchRequests());
  return (
    <ChangeButton
      action="Retry"
      name={`Retry ${target.label}`}
      change={retry}
      id={id}
      refusal={(error) => changeRefusal(error, target, "retry")}
      quality={target.quality}
    />
  );
}

/** Withdrawing can be undone by asking again, so it doesn't ask first. */
function Withdraw({ id, ...target }: Target) {
  const withdraw = useMutation<DescMessage, DescMessage>(
    target.changes.withdraw,
    useRefetchRequests(),
  );
  return (
    <ChangeButton
      action="Withdraw"
      name={`Withdraw from ${target.label}`}
      change={withdraw}
      id={id}
      refusal={(error) => changeRefusal(error, target, "withdraw from")}
    />
  );
}

function Decline({ id, ...target }: Target) {
  const { label } = target;
  const decline = useMutation<DescMessage, DescMessage>(
    target.changes.decline,
    useRefetchRequests(),
  );
  return (
    <ConfirmDialog
      action="Decline"
      name={`Decline ${label}`}
      title={`Decline ${label}?`}
      description={`Its Requesters see that it was declined, and why if you say. Anyone can ask for ${label} again.`}
      onConfirm={(typed) => decline.mutateAsync({ id, reason: formText(typed, "reason") })}
      refusal={(error) => changeRefusal(error, target, "decline")}
    >
      <FormTextarea label="Reason (optional)" name="reason" required={false} />
    </ConfirmDialog>
  );
}

function Cancel({ id, ...target }: Target) {
  const { title, changes } = target;
  const cancel = useMutation<DescMessage, DescMessage>(changes.cancel, useRefetchRequests());
  return (
    <ConfirmDialog
      action={`Cancel ${changes.noun}`}
      dismiss="Keep it"
      name={`Cancel ${changes.noun} for ${title}`}
      title={`Cancel the ${changes.noun} for ${title}?`}
      description={changes.cancelling}
      onConfirm={() => cancel.mutateAsync({ id })}
      refusal={(error) => changeRefusal(error, target, "cancel")}
    />
  );
}

/**
 * Why the Server refused to `change` the Request or Followed Show `target`, such as another User
 * changing it first.
 */
function changeRefusal(
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
      [RefusalReason.FOLLOWED_SHOW_NOT_PENDING]: decided,
      [RefusalReason.FOLLOWED_SHOW_NOT_FOUND]: gone,
      [RefusalReason.NOT_A_FOLLOWED_SHOW_REQUESTER]: noLongerRequester,
    },
  ).form;
}

function upperFirst(text: string) {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
