import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { FormTextarea } from "@/components/form-field";
import { Button } from "@/components/ui/button";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type Request, RequestService, RequestState } from "@/gen/nama/v1/request_pb";
import { requestsKey } from "@/lib/events";
import { formText, refusal } from "@/lib/forms";

/** Approve and Decline for a Pending Request, and Cancel for any, for a User who manages them. */
export function RequestActions({ request }: { request: Request }) {
  const target = { id: request.id, title: request.movie?.title ?? "this Movie" };
  return (
    // Pulled back by the buttons' own padding, so the first one's words line up with the title.
    <div className="-ms-4 mt-1 flex basis-full flex-wrap gap-1">
      {request.state === RequestState.PENDING && (
        <>
          <Approve {...target} />
          <Decline {...target} />
        </>
      )}
      <Cancel {...target} />
    </div>
  );
}

interface Target {
  id: bigint;
  /** The Movie's title, which names the Request to the User. */
  title: string;
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
    <>
      <Button
        variant="ghost"
        aria-label={`Approve ${title}`}
        disabled={approve.isPending}
        onClick={() => {
          approve.mutate({ id });
        }}
      >
        Approve
      </Button>
      {approve.isError && (
        <p role="alert" className="w-full ps-4 text-sm text-destructive">
          {changeRefusal(approve.error, title, "approve this Request")}
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
      [RefusalReason.REQUEST_NOT_FOUND]: { form: `The Request for ${title} is gone.` },
    },
  ).form;
}
