const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/** Cron's names for the days of the week, Sunday first. */
const WEEKDAY_NAMES = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

const NUMBER = /^\d+$/u;

/**
 * A Job's 5-field cron schedule in words, such as "Every day at 04:00 Server time": the Server
 * reads it in its own time zone, which may not be the browser's. A schedule of a shape it has no
 * words for is shown as written.
 */
export function scheduleWords(schedule: string): string {
  if (schedule === "") {
    return "Only when an Admin runs it";
  }
  const [minute = "", hour = "", day = "", month = "", weekday = ""] = schedule.split(/\s+/u);
  const words = month === "*" ? inWords({ minute, hour, day, weekday }) : "";
  return words === "" ? `On the schedule ${schedule}` : words;
}

/** The schedule in words, or blank for a shape this has none for. */
function inWords({
  minute,
  hour,
  day,
  weekday,
}: {
  minute: string;
  hour: string;
  day: string;
  weekday: string;
}) {
  if (day === "*" && weekday === "*" && /^\*(?:\/\d+)?$/u.test(hour)) {
    return everyFewHours(minute, hour);
  }
  const days = dayWords(day, weekday);
  if (days === "" || !NUMBER.test(minute) || !NUMBER.test(hour)) {
    return "";
  }
  return `${days} at ${hour.padStart(2, "0")}:${minute.padStart(2, "0")} Server time`;
}

/** The days a schedule fires on in words, or blank for a shape this has none for. */
function dayWords(day: string, weekday: string) {
  if (day === "*") {
    const names = weekday === "*" ? "day" : weekdayNames(weekday);
    return names === undefined ? "" : `Every ${names}`;
  }
  return weekday === "*" && NUMBER.test(day) ? `Every month on day ${Number(day)}` : "";
}

/** A schedule firing every hour, or every few hours, such as every 3, in words, or blank for a shape this has none for. */
function everyFewHours(minute: string, hour: string) {
  const step = Number(hour.slice(2));
  if (hour === "*" || step === 1) {
    return everyHour(minute);
  }
  const past = Number(minute) === 0 ? "on the hour" : `${Number(minute)} minutes past`;
  return NUMBER.test(minute) ? `Every ${step} hours from midnight, ${past}, Server time` : "";
}

/** A schedule firing every hour in words, or blank for a shape this has none for. */
function everyHour(minute: string) {
  if (minute === "*") {
    return "Every minute";
  }
  const step = /^\*\/(?<step>\d+)$/u.exec(minute)?.groups?.["step"];
  if (step !== undefined) {
    return Number(step) === 1 ? "Every minute" : `Every ${Number(step)} minutes`;
  }
  if (!NUMBER.test(minute)) {
    return "";
  }
  return Number(minute) === 0
    ? "Every hour, on the hour"
    : `Every hour, ${Number(minute)} minutes past`;
}

const AND = new Intl.ListFormat("en", { type: "conjunction" });

/**
 * The days of the week cron's weekday field `text` names, such as "Monday to Friday" for `1-5` or
 * "Saturday and Sunday" for `6,0`, or nothing for a shape this has no words for.
 */
function weekdayNames(text: string) {
  const items = text.split(",").map((item) => weekdayItem(item));
  return items.every((item) => item !== undefined) ? AND.format(items) : undefined;
}

/** One day, such as `1`, or a range of days, such as `1-5`, in words, or nothing if it's neither. */
function weekdayItem(item: string) {
  const [first = "", last, ...more] = item.split("-");
  const from = more.length === 0 ? weekdayName(first) : undefined;
  if (last === undefined || from === undefined) {
    return from;
  }
  const to = weekdayName(last);
  return to === undefined ? to : `${from} to ${to}`;
}

/** The day of the week cron names with `text`, a number with Sunday as 0 or 7, or a name. */
function weekdayName(text: string) {
  const index = NUMBER.test(text) ? Number(text) % 7 : WEEKDAY_NAMES.indexOf(text.toLowerCase());
  return WEEKDAYS[index];
}
