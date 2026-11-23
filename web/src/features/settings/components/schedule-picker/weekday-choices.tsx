import { Button } from "nama-ui/components/button";

const DAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
// Monday first, as most calendars set out a week.
const WEEK = [1, 2, 3, 4, 5, 6, 0];
const QUICK: [string, number[]][] = [
  ["Weekdays", [1, 2, 3, 4, 5]],
  ["Weekends", [0, 6]],
  ["Every day", [0, 1, 2, 3, 4, 5, 6]],
];

/** The days of the week a schedule runs on, each a button that's pressed while picked, and ways to pick several at once. */
export function WeekdayChoices({
  days,
  onChange,
}: {
  days: number[];
  onChange: (days: number[]) => void;
}) {
  const toggles = WEEK.map((day) => (
    <Button
      key={day}
      variant="secondary"
      aria-pressed={days.includes(day)}
      aria-label={DAYS[day]}
      onClick={() => {
        onChange(days.includes(day) ? days.filter((each) => each !== day) : [...days, day]);
      }}
    >
      {DAYS[day]?.slice(0, 3)}
    </Button>
  ));
  const quick = QUICK.map(([label, picked]) => (
    <Button
      key={label}
      variant="ghost"
      onClick={() => {
        onChange(picked);
      }}
    >
      {label}
    </Button>
  ));
  const picks = (
    <fieldset className="flex flex-wrap gap-1">
      <legend className="sr-only">Days</legend>
      {toggles}
    </fieldset>
  );
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
      {picks}
      <div className="flex flex-wrap">{quick}</div>
    </div>
  );
}
