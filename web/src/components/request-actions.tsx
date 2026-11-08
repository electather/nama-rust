import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { FormTextarea } from "@/components/form-field";
import { Button } from "@/components/ui/button";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type Request, RequestService, RequestState } from "@/gen/nama/v1/request_pb";
import { seasonName } from "@/lib/browse";
import { requestsKey } from "@/lib/events";
import { formText, refusal } from "@/lib/forms";

/**
 * What the signed-in User `you` may do to `request`: withdraw from it if it's their own and
 * Pending, and approve, decline, retry or cancel it if they `manage` Requests.
 */
export function RequestActions({
  request,
  you,
  manage,
}: {
  request: Request;
  you: bigint | undefined;
  manage: boolean;
}) {
  const target = { id: request.id, title: nameOf(request) };
  return (
    // Pulled back by the buttons' own padding, so the first one's words line up with the title.
    // Hidden when it has no buttons, so it adds no space to the row.
    <div className="-ms-4 mt-1 flex basis-full flex-wrap gap-1 empty:hidden">
      {canWithdraw(request, you) && <Withdraw {...target} />}
      {manage && <Manage state={request.state} {...target} />}
    </div>
  );
}

/** Whether `request` is Pending and the User `you` is one of its Requesters. */
function canWithdraw(request: Request, you: bigint | undefined) {
  return (
    request.state === RequestState.PENDING &&
    request.requesters.some(({ userId }) => userId === you)
  );
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
}

/** What `request` asks for, in words: a Movie's title, or a Show's with the Season's name. */
function nameOf({ movie, season }: Request) {
  if (season) return `${season.showTitle} ${seasonName(season.seasonNumber)}`;
  return movie?.title ?? "this Movie";
}

/** Mutation options that refetch the Requests once a change to one is done or refused. */
function useRefetchRequests() {
  const queryClient = useQueryClient();
  // Waits for the Requests to be read back, so the button stays busy until the change shows.
  return { onSettled: () => queryClient.invalidateQueries({ queryKey: requestsKey }) };
}

function Approve({ id, title }: Target) {
  const approve = useMutation(RequestService.method.approveRequest, useRefetchRequests());
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
function Withdraw({ id, title }: Target) {
  const withdraw = useMutation(RequestService.method.withdrawRequest, useRefetchRequests());
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

function Decline({ id, title }: Target) {
  const decline = useMutation(RequestService.method.declineRequest, useRefetchRequests());
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

function Cancel({ id, title }: Target) {
  const cancel = useMutation(RequestService.method.cancelRequest, useRefetchRequests());
  return (
    <ConfirmDialog
      action="Cancel Request"
      dismiss="Keep it"
      name={`Cancel Request for ${title}`}
      title={`Cancel the Request for ${title}?`}
      description="It’s removed for everyone who asked for it. This can’t be undone."
      onConfirm={() => cancel.mutateAsync({ id })}
      refusal={(error) => changeRefusal(error, title, "cancel this Request")}
    />
  );
}

/** Why the Server refused to change the Request for `title`, such as another User changing it first. */
function changeRefusal(error: unknown, title: string, attempt: string) {
  return refusal(
    error,
    attempt,
    {},
    {
      [RefusalReason.REQUEST_NOT_PENDING]: { form: `${title} was already approved or declined.` },
      [RefusalReason.REQUEST_NOT_FAILED]: { form: `${title} isn’t Failed any more.` },
      [RefusalReason.REQUEST_NOT_FOUND]: { form: `The Request for ${title} is gone.` },
      [RefusalReason.NOT_A_REQUESTER]: { form: `You’re no longer a Requester of ${title}.` },
    },
  ).form;
}
