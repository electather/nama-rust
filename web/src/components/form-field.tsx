import { type ComponentProps, type ReactNode, useId } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";

/** What a field shows around its control: its label, hint and the Server's error. */
export interface FieldText {
  label: string;
  name: string;
  /** Says what the field is for, under it. */
  hint?: string | undefined;
  /** The Server's reason for refusing what was typed, if it did. */
  error?: string | undefined;
}

/** A labelled, required text input in a native form, with its hint and the Server's error. */
export function FormField({ label, hint, error, ...input }: ComponentProps<"input"> & FieldText) {
  return (
    <Field label={label} hint={hint} error={error}>
      {(control) => <Input {...control} {...input} />}
    </Field>
  );
}

/** A `FormField` for several lines of text. */
export function FormTextarea({
  label,
  hint,
  error,
  ...textarea
}: ComponentProps<"textarea"> & FieldText) {
  return (
    <Field label={label} hint={hint} error={error}>
      {(control) => <Textarea {...control} {...textarea} />}
    </Field>
  );
}

function Field({
  label,
  hint,
  error,
  children,
}: Omit<FieldText, "name"> & {
  children: (control: {
    id: string;
    required: true;
    "aria-invalid": boolean;
    "aria-describedby": string;
  }) => ReactNode;
}) {
  const id = useId();
  return (
    <div className="flex flex-col gap-2">
      <label htmlFor={id} className="text-sm font-medium">
        {label}
      </label>
      {children({
        id,
        required: true,
        "aria-invalid": error !== undefined,
        "aria-describedby": `${id}-hint ${id}-error`,
      })}
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

/** The end of a form on a page: what submitting it did, why the Server refused, and its button. */
export function FormEnd({
  action,
  pending,
  done,
  refusal,
}: {
  /** Says what submitting does, such as "Change password". */
  action: string;
  pending: boolean;
  /** What the last submit did, once it's done. */
  done: string | undefined;
  /** Why the Server refused the form as a whole, if it did. */
  refusal: string | undefined;
}) {
  return (
    <>
      <output className="text-sm empty:hidden">{done}</output>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {refusal}
      </p>
      <div>
        <Button type="submit" disabled={pending}>
          {action}
        </Button>
      </div>
    </>
  );
}

/** A group of radio buttons named `name`, as `[value, label]` pairs. The first starts chosen. */
export function Choices({
  legend,
  name,
  choices,
}: {
  legend: string;
  name: string;
  choices: [value: string, label: string][];
}) {
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="mb-2 text-sm font-medium">{legend}</legend>
      {choices.map(([value, label], index) => (
        <label key={value} className="flex items-center gap-3 text-sm">
          <input
            type="radio"
            name={name}
            value={value}
            defaultChecked={index === 0}
            className="size-4 accent-foreground"
          />
          {label}
        </label>
      ))}
    </fieldset>
  );
}
