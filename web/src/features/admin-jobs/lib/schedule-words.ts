const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/** Cron's names for the days of the week, Sunday first. */
const WEEKDAY_NAMES = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

const NUMBER = /^\d+$/u;

/**
 * A Job's 5-field cron schedule in words, such as "Every day at 04:00", whose times are the
 * Server's. A schedule of a shape it has no words for is shown as written.
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
  if (hour === "*" && day === "*" && weekday === "*") {
    return everyHour(minute);
  }
  const days = dayWords(day, weekday);
  if (days === "" || !NUMBER.test(minute) || !NUMBER.test(hour)) {
    return "";
  }
  return `${days} at ${hour.padStart(2, "0")}:${minute.padStart(2, "0")}`;
}

/** The days a schedule fires on in words, or blank for a shape this has none for. */
function dayWords(day: string, weekday: string) {
  if (day === "*") {
    const name = weekday === "*" ? "day" : weekdayName(weekday);
    return name === undefined ? "" : `Every ${name}`;
  }
  return weekday === "*" && NUMBER.test(day) ? `Every month on day ${Number(day)}` : "";
}

/** A schedule firing every hour in words, or blank for a shape this has none for. */
function everyHour(minute: string) {
  if (minute === "*") {
    return "Every minute";
  }
  const step = /^\*\/(?<step>\d+)$/u.exec(minute)?.groups?.["step"];
  if (step !== undefined) {
    return `Every ${Number(step)} minutes`;
  }
  if (!NUMBER.test(minute)) {
    return "";
  }
  return Number(minute) === 0
    ? "Every hour, on the hour"
    : `Every hour, ${Number(minute)} minutes past`;
}

/** The day of the week cron names with `text`, a number with Sunday as 0 or 7, or a name. */
function weekdayName(text: string) {
  const index = NUMBER.test(text) ? Number(text) % 7 : WEEKDAY_NAMES.indexOf(text.toLowerCase());
  return WEEKDAYS[index];
}
