/** How often a schedule picked without typing cron runs; `custom` is cron typed as it is. */
export type PickKind = "minutes" | "hours" | "daily" | "weekly" | "monthly" | "custom";

/**
 * A Job's schedule as its picker holds it. Every kind's fields are kept, so switching kinds and
 * back keeps what was picked.
 */
export interface SchedulePick {
  kind: PickKind;
  /** `minutes`: how many minutes apart, one of `MINUTE_STEPS`. */
  every: number;
  /** `hours`: how many hours apart, one of `HOUR_STEPS`. */
  hours: number;
  /** `hours`: the minute past the hour, a multiple of 5. */
  minute: number;
  /** `daily`, `weekly` and `monthly`: the time of day, `HH:MM`, on the Server's clock. */
  time: string;
  /** `weekly`: the days of the week, Sunday as 0. */
  days: number[];
  /** `monthly`: the day of the month, 1 to 28, so it comes round in February too. */
  day: number;
  /** `custom`: the cron typed. */
  custom: string;
}

/** The kinds of schedule, in the order offered, with what they're called: short, so they fit one pill. */
export const pickKinds: [PickKind, string][] = [
  ["minutes", "Minutes"],
  ["hours", "Hours"],
  ["daily", "Daily"],
  ["weekly", "Weekly"],
  ["monthly", "Monthly"],
  ["custom", "Custom"],
];

/** The gaps `minutes` offers. */
export const MINUTE_STEPS = [5, 10, 15, 20, 30];

/** The gaps `hours` offers: the ones that divide a day evenly, as cron counts from midnight. */
export const HOUR_STEPS = [1, 2, 3, 4, 6, 8, 12];

const DEFAULTS: Omit<SchedulePick, "custom"> = {
  kind: "custom",
  every: 15,
  hours: 1,
  minute: 0,
  time: "04:00",
  days: [1, 2, 3, 4, 5],
  day: 1,
};

// Cron's minute and hour fields for one time of day, then each kind's shape after them.
const AT = String.raw`(?<minute>[1-5]?\d) (?<hour>1?\d|2[0-3])`;
const DAYS = String.raw`[0-7](?:-[0-7])?(?:,[0-7](?:-[0-7])?)*`;

type Groups = Partial<Record<string, string>>;

/** Each kind the picker shows, by the shape of its cron, and the fields that shape sets. */
const SHAPES: [RegExp, (groups: Groups) => Partial<SchedulePick>][] = [
  [
    /^\*\/(?<every>5|10|15|20|30) \* \* \* \*$/u,
    ({ every }) => ({ kind: "minutes", every: Number(every) }),
  ],
  [
    /^(?<minute>[1-5]?[05]) (?:\*|\*\/(?<hours>1|2|3|4|6|8|12)) \* \* \*$/u,
    ({ minute, hours = "1" }) => ({ kind: "hours", minute: Number(minute), hours: Number(hours) }),
  ],
  [new RegExp(`^${AT} \\* \\* \\*$`, "u"), (groups) => ({ kind: "daily", time: timeOf(groups) })],
  [
    new RegExp(`^${AT} \\* \\* (?<weekday>${DAYS})$`, "u"),
    (groups) => ({ kind: "weekly", time: timeOf(groups), days: weekdaysOf(groups["weekday"]) }),
  ],
  [
    new RegExp(`^${AT} (?<day>[1-9]|1\\d|2[0-8]) \\* \\*$`, "u"),
    (groups) => ({ kind: "monthly", time: timeOf(groups), day: Number(groups["day"]) }),
  ],
];

/** `cron` in the picker: its kind and fields, or `custom` for a shape the picker can't show. */
export function pickOf(cron: string): SchedulePick {
  const tidy = cron.trim().split(/\s+/u).join(" ");
  for (const [shape, fields] of SHAPES) {
    const groups = shape.exec(tidy)?.groups;
    if (groups) {
      return { ...DEFAULTS, custom: cron, ...fields(groups) };
    }
  }
  return { ...DEFAULTS, custom: cron };
}

/** `HH:MM` for the minute and hour a shape matched. */
function timeOf({ minute = "0", hour = "0" }: Groups) {
  return `${hour.padStart(2, "0")}:${minute.padStart(2, "0")}`;
}

/** The days cron's weekday field `text` names, such as `1-5` or `0,6`, Sunday as 0. */
function weekdaysOf(text = "") {
  const days = new Set<number>();
  for (const item of text.split(",")) {
    const [first = 0, last = first] = item.split("-").map(Number);
    for (let day = first; day <= last; day += 1) {
      days.add(day % 7);
    }
  }
  // oxlint-disable-next-line unicorn/no-array-sort -- sorts the array just built, and the lib target has no toSorted
  return [...days].sort((a, b) => a - b);
}

/** The cron `pick` stands for. */
export function cronOf(pick: SchedulePick) {
  return CRONS[pick.kind](pick);
}

const CRONS: Record<PickKind, (pick: SchedulePick) => string> = {
  minutes: ({ every }) => `*/${every} * * * *`,
  hours: ({ hours, minute }) => `${minute} ${hours === 1 ? "*" : `*/${hours}`} * * *`,
  daily: ({ time }) => `${clockFields(time)} * * *`,
  weekly: ({ time, days }) =>
    `${clockFields(time)} * * ${days.length === 7 ? "*" : dayRanges(days)}`,
  monthly: ({ time, day }) => `${clockFields(time)} ${day} * *`,
  custom: ({ custom }) => custom.trim().split(/\s+/u).join(" "),
};

/** Cron's minute and hour fields for `HH:MM`. */
function clockFields(time: string) {
  const [hour = 0, minute = 0] = time.split(":").map(Number);
  return `${minute} ${hour}`;
}

/** `days` as cron writes them, three or more in a row as a range, such as `1-5`. */
function dayRanges(days: number[]) {
  const runs: number[][] = [];
  // oxlint-disable-next-line unicorn/no-array-sort -- sorts a copy, and the lib target has no toSorted
  for (const day of [...days].sort((a, b) => a - b)) {
    const run = runs.at(-1);
    if (run?.at(-1) === day - 1) {
      run.push(day);
    } else {
      runs.push([day]);
    }
  }
  return runs.map((run) => (run.length >= 3 ? `${run[0]}-${run.at(-1)}` : run.join(","))).join(",");
}

const PROBLEMS: Partial<Record<PickKind, (pick: SchedulePick) => string | undefined>> = {
  weekly: ({ days }) => (days.length === 0 ? "Pick at least one day." : undefined),
  custom: ({ custom }) =>
    custom.trim() === "" ? "Type a schedule, or pick how often it runs." : undefined,
};

/** What's missing from `pick` before it can be saved, or nothing. The Server checks what's typed. */
export function pickProblem(pick: SchedulePick) {
  return PROBLEMS[pick.kind]?.(pick);
}

/** `pick` switched to `kind`. Custom starts from what was picked, so it can be changed a little. */
export function withKind(pick: SchedulePick, kind: PickKind): SchedulePick {
  const fromPicked = kind === "custom" && pickProblem(pick) === undefined;
  return { ...pick, kind, custom: fromPicked ? cronOf(pick) : pick.custom };
}
