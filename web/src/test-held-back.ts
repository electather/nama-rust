import { Code, ConnectError } from "@connectrpc/connect";

/**
 * An answer a fake Server holds back till the test calls `refuse`, which refuses it with
 * `unavailable`. `answer` serves as any method whose response may be empty.
 */
export function heldBack() {
  const held: { reject?: (error: ConnectError) => void } = {};
  const refused = new Promise<void>((_resolve, reject) => {
    held.reject = reject;
  });
  return {
    answer: async () => {
      await refused;
      return {};
    },
    refuse: () => {
      held.reject?.(new ConnectError("try again", Code.Unavailable));
    },
  };
}
