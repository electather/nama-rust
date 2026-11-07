import type {
  DescMessage,
  DescMethodUnary,
  MessageInitShape,
  MessageShape,
} from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import {
  callUnaryMethod,
  useMutation as useConnectMutation,
  useTransport,
} from "@connectrpc/connect-query";
import { type UseMutationOptions, useMutation } from "@tanstack/react-query";
import {
  type ReactNode,
  type RefObject,
  createContext,
  use,
  useCallback,
  useRef,
  useState,
} from "react";

import { ConfirmModal } from "@/components/confirm-dialog";
import { FormField } from "@/components/form-field";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { formText, refusal, refusalOf } from "@/lib/forms";

/** Asks the User to confirm it's them, resolving whether this Device then has Recent Authentication. */
type Ask = () => Promise<boolean>;

const AskContext = createContext<Ask>(() => Promise.resolve(false));

/**
 * Holds the one "Confirm it’s you" dialog that {@link useSensitiveMutation} opens, for the pages
 * inside it.
 */
export function ConfirmItsYou({ children }: { children: ReactNode }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [openings, setOpenings] = useState(0);
  const answer = useRef<(confirmed: boolean) => void>(undefined);
  const confirmed = useRef(false);

  const ask = useCallback(() => {
    confirmed.current = false;
    setOpenings((each) => each + 1);
    dialog.current?.showModal();
    return new Promise<boolean>((resolve) => {
      answer.current = resolve;
    });
  }, []);

  return (
    <AskContext value={ask}>
      {children}
      <ReauthenticateDialog
        dialog={dialog}
        openings={openings}
        onReauthenticated={() => {
          confirmed.current = true;
        }}
        onClose={() => answer.current?.(confirmed.current)}
      />
    </AskContext>
  );
}

/** Asks for the User's password, and gives this Device Recent Authentication with it. */
function ReauthenticateDialog({
  dialog,
  openings,
  onReauthenticated,
  onClose,
}: {
  dialog: RefObject<HTMLDialogElement | null>;
  openings: number;
  onReauthenticated: () => void;
  onClose: () => void;
}) {
  const reauthenticate = useConnectMutation(SignInService.method.reauthenticate, {
    onSuccess: onReauthenticated,
  });
  return (
    <ConfirmModal
      ref={dialog}
      openings={openings}
      onClose={onClose}
      action="Confirm"
      title="Confirm it’s you"
      description="Enter your password to go on. This keeps anyone else using this Device from making the change."
      onConfirm={(typed) =>
        reauthenticate.mutateAsync({
          proof: { case: "password", value: formText(typed, "password") },
        })
      }
      refusal={(error) =>
        refusal(
          error,
          "confirm it’s you",
          {},
          {
            [RefusalReason.WRONG_PASSWORD]: { form: "That isn’t your password." },
          },
        ).form
      }
    >
      <FormField label="Password" name="password" type="password" autoComplete="current-password" />
    </ConfirmModal>
  );
}

/**
 * Connect-query's `useMutation`, for an RPC that needs Recent Authentication. When the Server
 * refuses for want of it, the User is asked to confirm it's them, and the call is tried once more:
 * a second refusal is an ordinary error. Cancelling fails the call with `canceled`, which forms
 * don't show.
 */
export function useSensitiveMutation<I extends DescMessage, O extends DescMessage>(
  schema: DescMethodUnary<I, O>,
  options: Omit<
    UseMutationOptions<MessageShape<O>, ConnectError, MessageInitShape<I>>,
    "mutationFn"
  > = {},
) {
  const transport = useTransport();
  const ask = use(AskContext);
  return useMutation({
    ...options,
    mutationFn: async (input: MessageInitShape<I>) => {
      const call = () => callUnaryMethod(transport, schema, input);
      try {
        return await call();
      } catch (error) {
        if (refusalOf(error).reason !== RefusalReason.RECENT_AUTHENTICATION_REQUIRED) throw error;
        if (!(await ask())) throw new ConnectError("the User cancelled", Code.Canceled);
        return call();
      }
    },
  });
}
