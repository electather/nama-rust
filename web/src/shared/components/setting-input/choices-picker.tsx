import type { Choice, Setting } from "@/gen/nama/v1/plugin_pb";
import { PickChips } from "@/shared/components/setting-input/pick-chips";

/**
 * A several-choices setting's input: a chip per choice it holds, picked from the ones `offered`,
 * then one per held value not among them, marked no longer offered once `offered` is `current`, so
 * an admin sees why saving is refused and can remove it.
 */
export function ChoicesPicker({
  setting,
  offered,
  current,
  error,
}: {
  setting: Setting;
  offered: Choice[];
  /** Whether `offered` is what the Plugin offers now, not a Manifest's list a settings check replaces. */
  current: boolean;
  error: string | undefined;
}) {
  const held = heldOf(setting);
  const gone = held
    .filter((value) => !offered.some((choice) => choice.value === value))
    .map((value) => ({ value, label: current ? `${value} (no longer offered)` : value }));
  return (
    <PickChips
      label={setting.label}
      hint={hintOf(setting)}
      error={error}
      name={setting.key}
      items={[...offered.map(({ value, label }) => ({ value, label })), ...gone]}
      held={held}
    />
  );
}

/** What `setting` is for, then whether it's required. Picking none saves none, never its default. */
function hintOf({ description, required }: Setting) {
  return required ? `${description} Required.`.trim() : description;
}

/** The values `setting` holds, or its default's while nothing is stored. */
function heldOf({ value, defaultValue }: Setting) {
  const held = value ?? defaultValue;
  return held?.value.case === "texts" ? held.value.value.values : [];
}
