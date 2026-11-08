import type { ConnectError } from "@connectrpc/connect";
import { createConnectQueryKey, useMutation, useSuspenseQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { FormEnd, FormField } from "@/components/form-field";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { formText, onSubmitted, refusal, serverMessage } from "@/lib/forms";

const serverSettingsKey = createConnectQueryKey({
  schema: ServerService.method.getServerSettings,
  cardinality: "finite",
});

/** The form a User who manages settings sets the Server URL with, which Requests are linked at. */
export function ServerUrl() {
  const queryClient = useQueryClient();
  const { settings } = useSuspenseQuery(ServerService.method.getServerSettings, {}).data;
  const update = useMutation(ServerService.method.updateServerSettings, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: serverSettingsKey }),
  });
  const errors = serverUrlErrors(update.error);

  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        update.mutate({ settings: { url: formText(typed, "serverUrl") } });
      })}
    >
      <FormField
        label="Server URL"
        name="serverUrl"
        type="url"
        inputMode="url"
        autoComplete="url"
        placeholder="https://nama.example.com"
        required={false}
        defaultValue={settings?.url}
        hint="The address people use to reach this Server. Messages about Requests, such as Telegram’s, link to them there. Leave it empty for no links."
        error={errors.serverUrl}
      />
      <FormEnd
        action={update.isPending ? "Saving Server URL…" : "Save Server URL"}
        variant="ghost"
        pending={update.isPending}
        done={update.isSuccess ? "Saved." : undefined}
        refusal={errors.form}
      />
    </form>
  );
}

/** Why the Server refused the Server URL, under its field, or `{}` while it hasn't. */
function serverUrlErrors(error: ConnectError | null) {
  if (error === null) return {};
  return refusal(
    error,
    "save the Server URL",
    {},
    { [RefusalReason.NOT_A_URL]: { serverUrl: serverMessage(error) } },
  );
}
