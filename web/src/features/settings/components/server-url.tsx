import { useServerSettings } from "@/features/settings/data/use-server-settings";
import { useUpdateServerSettings } from "@/features/settings/data/use-update-server-settings";
import { serverUrlErrors } from "@/features/settings/lib/server-url-errors";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { FormField } from "@/shared/components/form-field/form-field";
import { formText, onSubmitted } from "@/shared/lib/forms";

/** The form a User who manages settings sets the Server URL with, which Requests and Invites link to. */
export function ServerUrl() {
  const { settings } = useServerSettings();
  const update = useUpdateServerSettings();
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
        hint="The address people use to reach this Server. Messages about Requests, such as Telegram’s, link to them there, and Invites’ links start with it. Leave it empty for no links, and no Invites."
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
