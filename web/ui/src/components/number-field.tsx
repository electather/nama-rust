import { NumberField as NumberFieldPrimitive } from "@base-ui/react/number-field";
import { cn } from "cn";
import { Minus, Plus } from "lucide-react";
import { type ComponentProps, useRef, useState } from "react";

// A number field with − and + at its end, built on Base UI's NumberField. DESIGN.md, "Inputs": as
// `input.tsx`, the Rim at 30% as a fill and a full Rim border, and 44px tall on a phone, where its
// buttons are pressed with a finger.

// What a press, an arrow key or the wheel commits, as opposed to leaving the field after typing.
const STEPS = new Set(["increment-press", "decrement-press", "keyboard", "wheel", "scrub"]);

const BUTTON =
  "flex aspect-square h-full shrink-0 items-center justify-center border-s border-input text-muted-foreground transition-colors outline-none hover:bg-muted hover:text-foreground focus-visible:bg-muted focus-visible:text-foreground disabled:pointer-events-none disabled:opacity-50";

/** A number as the field shows it, or empty for none. */
function shown(value: string | number | undefined) {
  return value === undefined || value === "" ? undefined : Number(value);
}

/** The text a number is given back as, empty for none. */
function text(value: number | null) {
  return value === null ? "" : String(value);
}

/** Where − or + (`reason`) steps to from the default `placeholder` shows, if it's a number. */
function stepFromDefault(reason: string, placeholder: string | undefined, step: number) {
  const from = Number(placeholder);
  if (Number.isNaN(from) || !reason.endsWith("-press")) {
    return null;
  }
  return reason === "increment-press" ? from + step : from - step;
}

/** `value`, kept within `min` and `max`. */
function clamp(value: number, min = -Infinity, max = Infinity) {
  return Math.min(Math.max(value, min), max);
}

/**
 * What a change to `next` leaves the field holding. From an empty field, which stands for the
 * default its `placeholder` shows, − and + step from that default, not from the minimum.
 */
function changedTo({
  next,
  current,
  reason,
  placeholder,
  bounds: { step, min, max },
}: {
  next: number | null;
  current: number | null;
  reason: string;
  placeholder: string | undefined;
  bounds: { step: number; min: number | undefined; max: number | undefined };
}) {
  const stepped = current === null ? stepFromDefault(reason, placeholder, step) : null;
  return stepped === null ? next : clamp(stepped, min, max);
}

/**
 * A whole-number field, taking and giving its value as text, as an `<input>` does, so a form sends
 * it under `name`. `onValueChange` hears each change, and `onStep` each step from − or +, an arrow
 * key or the wheel, once the field shows it: typing is saved as a text field's is, on leaving.
 * Empty, it stands for the default its `placeholder` shows, so − and + step from that.
 */
function NumberInput({
  value,
  defaultValue,
  min,
  max,
  step = 1,
  name,
  required,
  disabled,
  className,
  onValueChange,
  onStep,
  ...input
}: Omit<ComponentProps<"input">, "type" | "value" | "defaultValue" | "min" | "max" | "step"> & {
  value?: string | undefined;
  defaultValue?: string | number | undefined;
  min?: number | undefined;
  max?: number | undefined;
  step?: number | undefined;
  onValueChange?: ((value: string) => void) | undefined;
  onStep?: ((value: string, field: HTMLInputElement | null) => void) | undefined;
}) {
  const field = useRef<HTMLInputElement>(null);
  // Held here even when `value` isn't given, so a step from empty can start at the default.
  const [own, setOwn] = useState(shown(defaultValue) ?? null);
  const current = value === undefined ? own : (shown(value) ?? null);
  // What the last change left, which a step reports: Base UI's own commit knows only its step.
  const last = useRef(current);
  return (
    <NumberFieldPrimitive.Root
      value={current}
      min={min}
      max={max}
      step={step}
      // Sent by the field's own hidden input: the one typed in only shows the number.
      name={name}
      required={required}
      disabled={disabled}
      // Whole counts and years, which read wrong with a thousands separator.
      format={{ useGrouping: false, maximumFractionDigits: 0 }}
      className="w-full min-w-0"
      onValueChange={(next, details) => {
        const changed = changedTo({
          next,
          current,
          reason: details.reason,
          placeholder: input.placeholder,
          bounds: { step, min, max },
        });
        last.current = changed;
        setOwn(changed);
        onValueChange?.(text(changed));
      }}
      onValueCommitted={(_next, details) => {
        if (STEPS.has(details.reason)) {
          // After the field shows it, so a form saved from it sends the new number.
          setTimeout(() => {
            onStep?.(text(last.current), field.current);
          }, 0);
        }
      }}
    >
      <NumberFieldPrimitive.Group
        data-slot="number-field"
        className={cn(
          "flex h-11 w-full min-w-0 items-stretch overflow-hidden rounded-lg border border-input bg-input/30 text-base transition-colors focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 has-aria-invalid:border-destructive/50 has-aria-invalid:ring-3 has-aria-invalid:ring-destructive/40 md:h-8 data-disabled:cursor-not-allowed data-disabled:bg-muted",
          className,
        )}
      >
        <NumberFieldPrimitive.Input
          ref={field}
          className="w-full min-w-0 grow bg-transparent px-2.5 text-center tabular-nums outline-none placeholder:text-muted-foreground"
          {...input}
        />
        <NumberFieldPrimitive.Decrement aria-label="Decrease" className={BUTTON}>
          <Minus aria-hidden strokeWidth={1.5} className="size-4" />
        </NumberFieldPrimitive.Decrement>
        <NumberFieldPrimitive.Increment aria-label="Increase" className={BUTTON}>
          <Plus aria-hidden strokeWidth={1.5} className="size-4" />
        </NumberFieldPrimitive.Increment>
      </NumberFieldPrimitive.Group>
    </NumberFieldPrimitive.Root>
  );
}

export { NumberInput };
