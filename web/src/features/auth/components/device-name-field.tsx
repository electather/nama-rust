import { browserName } from "@/features/auth/lib/browser-name";
import { FormField } from "@/shared/components/form-field/form-field";

// Unlike signing in with a password, getting a code still needs the Device's name: the Server
// doesn't name it after the browser for a Sign-in Code.
/** The field that names this Device, starting with the browser's own name for itself. */
export function DeviceNameField({ error }: { error: string | undefined }) {
  return (
    <FormField
      label="Device name"
      name="deviceName"
      variant="lg"
      defaultValue={browserName(globalThis.navigator.userAgent)}
      hint="You’ll check this name on a Device where you’re already signed in."
      maxLength={64}
      error={error}
    />
  );
}
