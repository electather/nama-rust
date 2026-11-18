import type {
  DescMessage,
  DescMethodUnary,
  MessageInitShape,
  MessageShape,
} from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { type UseMutationOptions, useMutation } from "@tanstack/react-query";
import { createContext, use } from "react";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { refusalOf } from "@/shared/lib/forms";

/** Asks the User to confirm it's them, resolving whether this Device then has Recent Authentication. */
export type Ask = () => Promise<boolean>;

/** How `useSensitiveMutation` asks: `<ConfirmItsYou>` provides it, opening its dialog. */
// Fails loudly rather than as a cancel, which forms don't show: outside <ConfirmItsYou> a
// sensitive change would otherwise do nothing, silently.
// oxlint-disable-next-line require-await -- it only rejects, and a function returning a promise is async
export const AskContext = createContext<Ask>(async () => {
  throw new Error("useSensitiveMutation needs a <ConfirmItsYou> around it");
});

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
      const call = async () => {
        const response = await callUnaryMethod(transport, schema, input);
        return response;
      };
      try {
        return await call();
      } catch (error) {
        if (refusalOf(error).reason !== RefusalReason.RECENT_AUTHENTICATION_REQUIRED) {
          throw error;
        }
        if (!(await ask())) {
          throw new ConnectError("the User cancelled", Code.Canceled);
        }
        return call();
      }
    },
  });
}
