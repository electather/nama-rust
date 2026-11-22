import { Button } from "nama-ui/components/button";

import { CodeField } from "@/features/auth/components/code-field";
import { codeErrors } from "@/features/auth/lib/sign-in-errors";
import { useLookUpDevice } from "@/features/settings/data/use-look-up-device";
import { FormNotices } from "@/shared/components/sign-in-layout/form-notices";
import { type FormErrors, formText, onSubmitted } from "@/shared/lib/forms";

/**
 * The first step of signing in another Device from the Devices pane: the Sign-in Code it shows,
 * starting with `code` and why it was `refused`, if approving it just failed.
 */
export function DeviceCode({
  code,
  refused,
  onFound,
  onCancel,
}: {
  code: string | undefined;
  refused: FormErrors | undefined;
  onFound: (code: string, deviceName: string) => void;
  onCancel: () => void;
}) {
  const lookUp = useLookUpDevice();
  const errors = lookUp.error === null ? (refused ?? {}) : codeErrors(lookUp.error);
  const buttons = (
    <div className="flex gap-1">
      <Button type="submit" disabled={lookUp.isPending}>
        {lookUp.isPending ? "Looking it up…" : "Continue"}
      </Button>
      <Button type="button" variant="ghost" onClick={onCancel}>
        Cancel
      </Button>
    </div>
  );
  return (
    <form
      className="flex flex-col gap-3 py-2.5"
      onSubmit={onSubmitted((typed) => {
        const entered = formText(typed, "signInCode");
        lookUp.mutate(
          { code: entered },
          {
            onSuccess: ({ deviceName }) => {
              onFound(entered, deviceName);
            },
          },
        );
      })}
    >
      <CodeField
        label="Sign-in Code"
        name="signInCode"
        length={8}
        placeholder="XXXX-XXXX"
        defaultValue={code}
        hint="It’s on the screen of the Device you’re signing in."
        error={errors.signInCode}
        // The field is what Sign in a Device opened, so typing goes straight in.
        // oxlint-disable-next-line jsx-a11y/no-autofocus -- it replaces the button that was pressed
        autoFocus
      />
      <FormNotices errors={errors} />
      {buttons}
    </form>
  );
}
