import { useId } from "react";

import { OffsetStepper } from "@/features/player/components/offset-stepper";

/**
 * A Subtitle Offset of `offsetMs`, with − and + that `shift` it a step either way, or, without
 * `shift`, disabled, saying a burned-in subtitle can't be offset.
 */
export function OffsetControls({
  offsetMs,
  shift,
}: {
  offsetMs: number;
  shift?: (by: number) => void;
}) {
  const labelId = useId();
  const noteId = useId();
  const burnedIn = shift === undefined;
  return (
    <fieldset
      aria-labelledby={labelId}
      aria-describedby={burnedIn ? noteId : undefined}
      disabled={burnedIn}
      className="sticky -bottom-1 flex flex-col gap-1 bg-popover px-2.5 pt-2 pb-2.5"
    >
      <span id={labelId} className="font-semibold">
        Offset
      </span>
      <OffsetStepper offsetMs={offsetMs} shift={shift} />
      {burnedIn ? (
        <p id={noteId} className="text-sm text-muted-foreground">
          A burned-in subtitle can’t be offset.
        </p>
      ) : null}
    </fieldset>
  );
}
