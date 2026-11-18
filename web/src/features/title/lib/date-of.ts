const dates = new Intl.DateTimeFormat("en", { dateStyle: "medium", timeZone: "UTC" });

/** A `YYYY-MM-DD` date in plain words, such as "Feb 3, 2022". */
export function dateOf(date: string | undefined) {
  // A bare date parses as midnight UTC, so it's formatted in UTC too.
  return date === undefined ? undefined : dates.format(new Date(date));
}
