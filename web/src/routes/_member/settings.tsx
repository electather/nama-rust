import { Code } from "@connectrpc/connect";
import { createQueryOptions, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { DeviceList } from "@/components/device-list";
import { FormEnd, FormField } from "@/components/form-field";
import { Page } from "@/components/page";
import { UserService } from "@/gen/nama/v1/user_pb";
import { type FormErrors, formText, onSubmitted, refusal, serverMessage } from "@/lib/forms";
import { devicesKey } from "@/lib/users";

export const Route = createFileRoute("/_member/settings")({
  loader: ({ context: { queryClient, transport } }) =>
    queryClient.query({
      ...createQueryOptions(UserService.method.listDevices, {}, { transport }),
      staleTime: "static",
    }),
  component: Settings,
});

function Settings() {
  return (
    <Page heading="Settings">
      <section aria-labelledby="password" className="mt-10 max-w-sm">
        <h2 id="password" className="font-medium">
          Password
        </h2>
        <ChangePassword />
      </section>
      <section aria-labelledby="devices" className="mt-12 max-w-2xl">
        <h2 id="devices" className="font-medium">
          Devices
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Where you’re signed in. Revoking a Device signs it out.
        </p>
        <div className="mt-4">
          <DeviceList />
        </div>
      </section>
    </Page>
  );
}

function ChangePassword() {
  const queryClient = useQueryClient();
  const change = useMutation(UserService.method.changePassword, {
    // The Server signed out every other Device.
    onSuccess: () => queryClient.invalidateQueries({ queryKey: devicesKey }),
  });
  const errors = change.error ? changePasswordErrors(change.error) : {};

  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed, form) => {
        change.mutate(
          {
            currentPassword: formText(typed, "currentPassword"),
            newPassword: formText(typed, "newPassword"),
          },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      })}
    >
      <FormField
        label="Current password"
        name="currentPassword"
        type="password"
        autoComplete="current-password"
        error={errors.currentPassword}
      />
      <FormField
        label="New password"
        name="newPassword"
        type="password"
        autoComplete="new-password"
        error={errors.newPassword}
      />
      <FormEnd
        action={change.isPending ? "Changing password…" : "Change password"}
        pending={change.isPending}
        done={
          change.isSuccess ? "Password changed. Your other Devices were signed out." : undefined
        }
        refusal={errors.form}
      />
    </form>
  );
}

function changePasswordErrors(error: unknown): FormErrors {
  return refusal(error, "change your password", {
    [Code.Unauthenticated]: { currentPassword: "That isn’t your current password." },
    [Code.InvalidArgument]: { newPassword: serverMessage(error) },
  });
}
