import type { Choice, Setting } from "@/gen/nama/v1/plugin_pb";
import { PickBox } from "@/shared/components/setting-input/pick-box";
import { PickList } from "@/shared/components/setting-input/pick-list";

/**
 * A several-choices setting's input: a checkbox per choice `offered`, ticked for the ones it holds,
 * then a ticked one per held value no longer offered, so an admin sees why saving is refused and
 * can unpick it.
 */
export function ChoicesPicker({
  setting,
  offered,
  error,
}: {
  setting: Setting;
  offered: Choice[];
  error: string | undefined;
}) {
  const held = heldOf(setting);
  const gone = held.filter((value) => !offered.some((choice) => choice.value === value));
  return (
    <PickList label={setting.label} hint={hintOf(setting)} error={error}>
      {(box) => [
        ...offered.map(({ value, label }) => (
          <PickBox
            key={value}
            name={setting.key}
            value={value}
            checked={held.includes(value)}
            {...box}
          >
            {label}
          </PickBox>
        )),
        ...gone.map((value) => (
          <PickBox key={value} name={setting.key} value={value} checked {...box}>
            {value} <span className="text-muted-foreground">(no longer offered)</span>
          </PickBox>
        )),
      ]}
    </PickList>
  );
}

/** What `setting` is for, then whether it's required. Picking none saves none, never its default. */
function hintOf({ description, required }: Setting) {
  return required ? `${description} Required.`.trim() : description;
}

/** The values `setting` holds, or its default's while nothing is stored, which the form then saves. */
function heldOf({ value, defaultValue }: Setting) {
  const held = value ?? defaultValue;
  return held?.value.case === "texts" ? held.value.value.values : [];
}
