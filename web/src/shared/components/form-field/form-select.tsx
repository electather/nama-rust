import { ChevronDown } from "lucide-react";
import type { ComponentProps } from "react";

import { Field, type FieldText } from "@/shared/components/form-field/field";
import { RowField } from "@/shared/components/form-field/row-field";

const LOOKS = {
  stacked: { Frame: Field, size: "h-10 bg-transparent ps-3 pe-9" },
  // Beside the fields of other rows, so as tall and as filled as `ui/input.tsx`.
  row: { Frame: RowField, size: "h-8 bg-input/30 ps-2.5 pe-8" },
};

/**
 * A labelled native dropdown in a native form, with its hint and the Server's error (DESIGN.md,
 * "Inputs"). As a `row` it's drawn as a setting's row, for a form in Settings. The browser's own
 * arrow is swapped for ours: once a dropdown has a border and a fill of its own, the browser draws
 * its arrow hard against the edge, across the rounded corner.
 */
export function FormSelect({
  label,
  hint,
  error,
  row = false,
  ...select
}: ComponentProps<"select"> & FieldText & { row?: boolean }) {
  const { Frame, size } = LOOKS[row ? "row" : "stacked"];
  return (
    <Frame label={label} hint={hint} error={error}>
      {(control) => (
        <span className="relative flex w-full min-w-0">
          <select
            {...control}
            {...select}
            className={`w-full min-w-0 appearance-none truncate rounded-lg border border-input text-base transition-colors outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/25 disabled:cursor-not-allowed disabled:bg-muted disabled:text-muted-foreground aria-invalid:border-destructive ${size}`}
          />
          <ChevronDown
            aria-hidden
            className="pointer-events-none absolute inset-y-0 inset-e-2.5 my-auto size-4 text-muted-foreground"
          />
        </span>
      )}
    </Frame>
  );
}
