import type { ConnectError } from "@connectrpc/connect";

import { PluginConnectionState } from "@/gen/nama/v1/user_pb";
import { refusal } from "@/shared/lib/forms";

/** Why the `attempt` failed with `error`, if it did. */
export function failure(error: ConnectError | null, attempt: string) {
  return error ? refusal(error, attempt, {}).form : undefined;
}

/** Where the User's connection to the Plugin `name` is, in plain words. */
export function status(state: PluginConnectionState, name: string) {
  if (state === PluginConnectionState.WAITING) {
    return `Open ${name} and press Start.`;
  }
  return state === PluginConnectionState.CONNECTED ? "Connected" : "Not connected";
}

// ponytail: a Plugin's id capitalized, which fits Telegram; give Plugins a display name once one
// doesn't read right.
export function pluginName(id: string) {
  return `${id.charAt(0).toUpperCase()}${id.slice(1)}`;
}
