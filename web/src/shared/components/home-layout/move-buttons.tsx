import { ArrowDown, ArrowUp } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { useRef } from "react";
import { flushSync } from "react-dom";

/** The arrows that move the row called `label`, at `index` of `count`, one place up or down. */
export function MoveButtons({
  label,
  place: { index, count },
  onMove,
}: {
  label: string;
  place: { index: number; count: number };
  onMove: (from: number, before: number) => void;
}) {
  const up = useRef<HTMLButtonElement>(null);
  const down = useRef<HTMLButtonElement>(null);
  const buttons = [
    {
      ref: up,
      other: down,
      direction: "up",
      Icon: ArrowUp,
      before: index - 1,
      disabled: index === 0,
    },
    {
      ref: down,
      other: up,
      direction: "down",
      Icon: ArrowDown,
      before: index + 2,
      disabled: index === count - 1,
    },
  ];
  return (
    // Shown on the row under the pointer or holding focus, so each row at rest is its name and
    // switch; always on a touch screen, which has no hover.
    <div className="flex opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100 motion-reduce:transition-none pointer-coarse:opacity-100">
      {buttons.map(({ ref, other, direction, Icon, before, disabled }) => (
        <Button
          key={direction}
          ref={ref}
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={`Move ${label} ${direction}`}
          disabled={disabled}
          onClick={() => {
            flushSync(() => {
              onMove(index, before);
            });
            // Reordering can move this row's node, which drops its focus; at the end, this arrow
            // is disabled, so the other one takes it.
            (ref.current?.disabled === true ? other : ref).current?.focus();
          }}
        >
          <Icon aria-hidden />
        </Button>
      ))}
    </div>
  );
}
