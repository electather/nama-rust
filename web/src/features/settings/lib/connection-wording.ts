import type { ConnectError } from "@connectrpc/connect";

import { PluginConnectionState } from "@/gen/nama/v1/user_pb";
import { refusal } from "@/shared/lib/forms";

/** Why the `attempt` failed with `error`, if it did. */
export function failure(error: ConnectError | null, attempt: string) {
  return error ? refusal(error, attempt, {}).form : undefined;
}

/** What the Plugin `name` does for the User, where their connection to it is. */
export function about(state: PluginConnectionState, name: string) {
  if (state === PluginConnectionState.WAITING) {
    return `Finish in ${name}. This updates on its own.`;
  }
  return state === PluginConnectionState.CONNECTED
    ? "Messages you when one of your Requests changes state."
    : "Can message you when one of your Requests changes state.";
}

// ponytail: a Plugin's id capitalized, which fits Telegram; give Plugins a display name once one
// doesn't read right.
export function pluginName(id: string) {
  return `${id.charAt(0).toUpperCase()}${id.slice(1)}`;
}
