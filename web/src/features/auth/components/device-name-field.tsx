import { FormField } from "@/shared/components/form-field/form-field";

// Unlike signing in with a password, getting a code still needs the Device's name: the Server
// doesn't name it after the browser for a Sign-in Code.
/** The field that names this Device. */
export function DeviceNameField({ error }: { error: string | undefined }) {
  return (
    <FormField
      label="Device name"
      name="deviceName"
      hint="So you can tell your Devices apart in Settings."
      placeholder="Living room laptop"
      maxLength={64}
      error={error}
    />
  );
}
