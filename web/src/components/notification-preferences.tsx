import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { NotificationKind, type NotificationPreference, UserService } from "@/gen/nama/v1/user_pb";
import { refusal } from "@/shared/lib/forms";

const preferencesKey = createConnectQueryKey({
  schema: UserService.method.listNotificationPreferences,
  cardinality: "finite",
});

/** What each kind of notification tells the User, as its checkbox says it. */
const labels: Record<NotificationKind, string> = {
  [NotificationKind.UNSPECIFIED]: "",
  [NotificationKind.APPROVED]: "Your Request is approved",
  [NotificationKind.DECLINED]: "Your Request is declined",
  [NotificationKind.FAILED]: "Your Request Failed",
  [NotificationKind.PARTIALLY_AVAILABLE]: "Your Request is Partially Available",
  [NotificationKind.AVAILABLE]: "Your Request is Available",
  [NotificationKind.CANCELLED_BY_DELETION]:
    "Your Request is cancelled because its media was deleted",
  [NotificationKind.NEW_PENDING]: "Someone makes a Request that needs approving",
};

/**
 * The kinds of Request notification the signed-in User gets on every app they connected, each a
 * checkbox that saves as it's ticked.
 */
export function NotificationPreferences() {
  const { data } = useQuery(UserService.method.listNotificationPreferences, {});
  if (!data) {
    return null;
  }
  return (
    <section aria-labelledby="notifications" className="mt-12 max-w-2xl">
      <h2 id="notifications" className="font-medium">
        Notifications
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        What Nama messages you about, in every app you connect.
      </p>
      <ul className="mt-4 flex flex-col gap-3">
        {data.preferences.map((preference) => (
          <PreferenceCheckbox key={preference.kind} preference={preference} />
        ))}
      </ul>
    </section>
  );
}

function PreferenceCheckbox({ preference: { kind, on } }: { preference: NotificationPreference }) {
  const queryClient = useQueryClient();
  const set = useMutation(UserService.method.setNotificationPreference, {
    onSettled: () => queryClient.invalidateQueries({ queryKey: preferencesKey }),
  });
  // Shows the choice being saved until the list comes back with it.
  const checked = set.isPending ? set.variables.on : on;
  return (
    <li>
      <label className="flex items-start gap-3 text-sm">
        <input
          type="checkbox"
          checked={checked === true}
          onChange={(event) => {
            set.mutate({ kind, on: event.target.checked });
          }}
          className="mt-0.5 size-4 shrink-0 accent-foreground"
        />
        {labels[kind]}
      </label>
      {set.error && (
        <p role="alert" className="mt-1 text-sm text-destructive">
          {refusal(set.error, "save that", {}).form}
        </p>
      )}
    </li>
  );
}
