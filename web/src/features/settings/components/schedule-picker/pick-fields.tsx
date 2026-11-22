import type { ReactNode } from "react";

import {
  HOUR_STEPS,
  MINUTE_STEPS,
  type PickKind,
  type SchedulePick,
} from "@/features/admin-jobs/lib/schedule-picks";
import { CustomField } from "@/features/settings/components/schedule-picker/custom-field";
import { PickSelect } from "@/features/settings/components/schedule-picker/pick-select";
import { TimeField } from "@/features/settings/components/schedule-picker/time-field";
import { WeekdayChoices } from "@/features/settings/components/schedule-picker/weekday-choices";

/** What the fields of the kind picked are given. */
export interface PickFieldsProps {
  pick: SchedulePick;
  invalid: boolean;
  onChange: (part: Partial<SchedulePick>) => void;
}

const ORDINAL = new Intl.PluralRules("en", { type: "ordinal" });
const SUFFIX: Partial<Record<Intl.LDMLPluralRule, string>> = { one: "st", two: "nd", few: "rd" };
const ordinal = (day: number) => `${day}${SUFFIX[ORDINAL.select(day)] ?? "th"}`;

const MINUTES = MINUTE_STEPS.map((step) => ({ value: step, label: `${step} minutes` }));
const HOURS = HOUR_STEPS.map((step) => ({
  value: step,
  label: step === 1 ? "hour" : `${step} hours`,
}));
const PAST = Array.from({ length: 12 }, (_, index) => index * 5).map((minute) => ({
  value: minute,
  label: minute === 0 ? "on the hour" : `${minute} minutes past`,
}));
const DATES = Array.from({ length: 28 }, (_, index) => index + 1).map((day) => ({
  value: day,
  label: ordinal(day),
}));

const time = ({ pick, onChange }: PickFieldsProps) => (
  <TimeField
    value={pick.time}
    onChange={(picked) => {
      onChange({ time: picked });
    }}
  />
);

/** What each kind asks for once it's picked. */
const FIELDS: Record<PickKind, (props: PickFieldsProps) => ReactNode> = {
  minutes: ({ pick, onChange }) => (
    <PickSelect
      label="Every"
      options={MINUTES}
      value={pick.every}
      onChange={(every) => {
        onChange({ every });
      }}
    />
  ),
  hours: ({ pick, onChange }) => (
    <>
      <PickSelect
        label="Every"
        options={HOURS}
        value={pick.hours}
        onChange={(hours) => {
          onChange({ hours });
        }}
      />
      <PickSelect
        label="at"
        options={PAST}
        value={pick.minute}
        onChange={(minute) => {
          onChange({ minute });
        }}
      />
    </>
  ),
  daily: time,
  weekly: (props) => (
    <>
      <WeekdayChoices
        days={props.pick.days}
        onChange={(days) => {
          props.onChange({ days });
        }}
      />
      {time(props)}
    </>
  ),
  monthly: (props) => (
    <>
      <PickSelect
        label="On the"
        options={DATES}
        value={props.pick.day}
        onChange={(day) => {
          props.onChange({ day });
        }}
      />
      {time(props)}
    </>
  ),
  custom: ({ pick, invalid, onChange }) => (
    <CustomField
      value={pick.custom}
      invalid={invalid}
      onChange={(custom) => {
        onChange({ custom });
      }}
    />
  ),
};

/** The fields of the kind of schedule picked, in a row that wraps. */
export function PickFields(props: PickFieldsProps) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2.5">
      {FIELDS[props.pick.kind](props)}
    </div>
  );
}
