import { type ComponentProps, useId } from "react";

import { Input } from "@/components/ui/input";

/** A labelled, required text input in a native form, with its hint and the Server's error. */
export function FormField({
  label,
  hint,
  error,
  ...input
}: ComponentProps<"input"> & {
  label: string;
  name: string;
  /** Says what the field is for, under it. */
  hint?: string;
  /** The Server's reason for refusing what was typed, if it did. */
  error?: string | undefined;
}) {
  const id = useId();
  return (
    <div className="flex flex-col gap-2">
      <label htmlFor={id} className="text-sm font-medium">
        {label}
      </label>
      <Input
        id={id}
        required
        aria-invalid={error !== undefined}
        aria-describedby={`${id}-hint ${id}-error`}
        {...input}
      />
      <p id={`${id}-hint`} className="text-sm text-muted-foreground empty:hidden">
        {hint}
      </p>
      {/* Always there, so screen readers announce an error as it appears. */}
      <p id={`${id}-error`} role="alert" className="text-sm text-destructive empty:hidden">
        {error}
      </p>
    </div>
  );
}

/** The name of the Device signing in, which both setting up and signing in ask for. */
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
