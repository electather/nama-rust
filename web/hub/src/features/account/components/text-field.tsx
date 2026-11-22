import { Input } from "nama-ui/components/input";
import { type ComponentProps, useId } from "react";

/** A labelled input, with a hint under it until the Hub's error for it takes its place. */
export function TextField({
  label,
  hint,
  error,
  ...input
}: ComponentProps<typeof Input> & {
  label: string;
  hint?: string | undefined;
  /** The Hub's reason for refusing what was typed, if it did. */
  error: string | undefined;
}) {
  const id = useId();
  return (
    <div className="flex flex-col gap-2">
      <label htmlFor={id} className="text-sm font-medium">
        {label}
      </label>
      <Input
        id={id}
        variant="lg"
        required
        aria-invalid={error !== undefined}
        aria-describedby={`${id}-note`}
        {...input}
      />
      {/* Always there, so a screen reader announces an error as it appears. */}
      <p
        id={`${id}-note`}
        role={error === undefined ? undefined : "alert"}
        className={
          error === undefined
            ? "text-xs text-muted-foreground empty:hidden"
            : "text-sm text-destructive"
        }
      >
        {error ?? hint}
      </p>
    </div>
  );
}
