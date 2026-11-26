import { type HubWording, hubRefusalReason, sayHubRefusal } from "@/features/auth/lib/hub-errors";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";

/** Whether the Hub refused for want of Recent Authentication, which signing in again gives. */
export function needsRecentSignIn(error: unknown) {
  return hubRefusalReason(error) === RefusalReason.RECENT_AUTHENTICATION_REQUIRED;
}

/** How long ago the Account signed in, as the Hub's Recent Authentication refusal on `error` says. */
export function signedInTooLongAgo(error: unknown) {
  return sayHubRefusal(error, recentWording, "You signed in a while ago.").form;
}

const recentWording: HubWording = {
  [RefusalReason.RECENT_AUTHENTICATION_REQUIRED]: (param) => ({
    form: `You signed in over ${param("recentMinutes")} minutes ago.`,
  }),
};
