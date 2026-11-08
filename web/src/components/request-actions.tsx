import type { DescMessage } from "@bufbuild/protobuf";
import { useMutation } from "@connectrpc/connect-query";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { FormTextarea } from "@/components/form-field";
import { useRefetchRequests } from "@/components/request-state";
import { Button } from "@/components/ui/button";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type FollowedShow,
  type Request,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { seasonName } from "@/lib/browse";
import { formText, refusal } from "@/lib/forms";

/** The RPCs that change a Request by its ID, and what cancelling one does. */
const requestChanges = {
  approve: RequestService.method.approveRequest,
  decline: RequestService.method.declineRequest,
  cancel: RequestService.method.cancelRequest,
  withdraw: RequestService.method.withdrawRequest,
  cancelling: "It’s removed for everyone who asked for it. This can’t be undone.",
};

/** The RPCs that change a Followed Show by its ID, and what cancelling one does. */
const followedShowChanges = {
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
  /** What the Request asks for, which names it to the User. */
  title: string;
  /**
   * Both kinds' RPCs take an `id`, and Decline's a `reason` too, but a union of the two can't be
   * inferred, so the components call them as taking any message.
   */
  changes: typeof requestChanges | typeof followedShowChanges;
}

function targetOf(request: Request | FollowedShow): Target {
  const { id } = request;
  return request.$typeName === "nama.v1.FollowedShow"
    ? { id, title: `${request.showTitle} Whole Show`, changes: followedShowChanges }
    : { id, title: nameOf(request), changes: requestChanges };
}

/** What `request` asks for, in words: a Movie's title, or a Show's with the Season's name. */
function nameOf({ movie, season }: Request) {
  if (season) return `${season.showTitle} ${seasonName(season.seasonNumber)}`;
  return movie?.title ?? "this Movie";
}

function Approve({ id, title, changes }: Target) {
  const approve = useMutation<DescMessage, DescMessage>(changes.approve, useRefetchRequests());
  return (
    <ChangeButton
      action="Approve"
      name={`Approve ${title}`}
      change={approve}
      id={id}
      refusal={(error) => changeRefusal(error, title, "approve this Request")}
    />
  );
}

/** Hands a Failed Request to its Fulfiller again. */
function Retry({ id, title }: Target) {
  const retry = useMutation(RequestService.method.retryRequest, useRefetchRequests());
  return (
    <ChangeButton
      action="Retry"
      name={`Retry ${title}`}
      change={retry}
      id={id}
      refusal={(error) => changeRefusal(error, title, "retry this Request")}
    />
  );
}

/** Withdrawing can be undone by asking again, so it doesn't ask first. */
function Withdraw({ id, title, changes }: Target) {
  const withdraw = useMutation<DescMessage, DescMessage>(changes.withdraw, useRefetchRequests());
  return (
    <ChangeButton
      action="Withdraw"
      name={`Withdraw from ${title}`}
      change={withdraw}
      id={id}
      refusal={(error) => changeRefusal(error, title, "withdraw from this Request")}
    />
  );
}

interface ChangeButtonProps {
  action: string;
  /** The button's accessible name, which says which Request it changes. */
  name: string;
  /** The mutation the button calls with the Request's ID. */
  change: {
    mutate: (input: { id: bigint }) => void;
    isPending: boolean;
    isError: boolean;
    error: unknown;
  };
  id: bigint;
  refusal: (error: unknown) => string | undefined;
}

/** A button that changes a Request at once, and says why if the Server refuses. */
function ChangeButton({ action, name, change, id, refusal: whyRefused }: ChangeButtonProps) {
  return (
    <>
      <Button
        variant="ghost"
        aria-label={name}
        disabled={change.isPending}
        onClick={() => {
          change.mutate({ id });
        }}
      >
        {action}
      </Button>
      {change.isError && (
        <p role="alert" className="w-full ps-4 text-sm text-destructive">
          {whyRefused(change.error)}
        </p>
      )}
    </>
  );
}

function Decline({ id, title, changes }: Target) {
  const decline = useMutation<DescMessage, DescMessage>(changes.decline, useRefetchRequests());
  return (
    <ConfirmDialog
      action="Decline"
      name={`Decline ${title}`}
      title={`Decline ${title}?`}
      description={`Its Requesters see that it was declined, and why if you say. Anyone can ask for ${title} again.`}
      onConfirm={(typed) => decline.mutateAsync({ id, reason: formText(typed, "reason") })}
      refusal={(error) => changeRefusal(error, title, "decline this Request")}
    >
      <FormTextarea label="Reason (optional)" name="reason" required={false} />
    </ConfirmDialog>
  );
}

function Cancel({ id, title, changes }: Target) {
  const cancel = useMutation<DescMessage, DescMessage>(changes.cancel, useRefetchRequests());
  return (
    <ConfirmDialog
      action="Cancel Request"
      dismiss="Keep it"
      name={`Cancel Request for ${title}`}
      title={`Cancel the Request for ${title}?`}
      description={changes.cancelling}
      onConfirm={() => cancel.mutateAsync({ id })}
      refusal={(error) => changeRefusal(error, title, "cancel this Request")}
    />
  );
}

/** Why the Server refused to change the Request for `title`, such as another User changing it first. */
function changeRefusal(error: unknown, title: string, attempt: string) {
  // A Followed Show has reasons of its own, which say the same to its Requesters.
  const decided = { form: `${title} was already approved or declined.` };
  const gone = { form: `The Request for ${title} is gone.` };
  const noLongerRequester = { form: `You’re no longer a Requester of ${title}.` };
  return refusal(
    error,
    attempt,
    {},
    {
      [RefusalReason.REQUEST_NOT_PENDING]: decided,
      [RefusalReason.REQUEST_NOT_FOUND]: gone,
      [RefusalReason.NOT_A_REQUESTER]: noLongerRequester,
      [RefusalReason.REQUEST_NOT_FAILED]: { form: `${title} isn’t Failed any more.` },
      [RefusalReason.FOLLOWED_SHOW_NOT_PENDING]: decided,
      [RefusalReason.FOLLOWED_SHOW_NOT_FOUND]: gone,
      [RefusalReason.NOT_A_FOLLOWED_SHOW_REQUESTER]: noLongerRequester,
    },
  ).form;
}
