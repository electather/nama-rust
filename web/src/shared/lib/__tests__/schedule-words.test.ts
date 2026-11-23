import { describe, expect, test } from "vite-plus/test";

import { scheduleWords } from "@/shared/lib/schedule-words";

describe("a Job's schedule in words", () => {
  test.each([
    ["0 4 * * *", "Every day at 04:00 Server time"],
    ["10 4 * * *", "Every day at 04:10 Server time"],
    ["30 3 * * 0", "Every Sunday at 03:30 Server time"],
    ["30 3 * * 7", "Every Sunday at 03:30 Server time"],
    ["0 5 * * mon", "Every Monday at 05:00 Server time"],
    ["0 4 * * 1-5", "Every Monday to Friday at 04:00 Server time"],
    ["0 9 * * 6,0", "Every Saturday and Sunday at 09:00 Server time"],
    ["0 2 1 * *", "Every month on day 1 at 02:00 Server time"],
    ["0 * * * *", "Every hour, on the hour"],
    ["15 * * * *", "Every hour, 15 minutes past"],
    ["0 */6 * * *", "Every 6 hours from midnight, on the hour, Server time"],
    ["30 */2 * * *", "Every 2 hours from midnight, 30 minutes past, Server time"],
    ["0 */1 * * *", "Every hour, on the hour"],
    ["*/15 * * * *", "Every 15 minutes"],
    ["* * * * *", "Every minute"],
    ["*/1 * * * *", "Every minute"],
  ])("%s reads %s", (schedule, words) => {
    expect(scheduleWords(schedule)).toStrictEqual(words);
  });

  test("a schedule it has no words for is shown as written", () => {
    expect(scheduleWords("0 4 1-7 * 1")).toBe("On the schedule 0 4 1-7 * 1");
  });

  test("a Job without a schedule runs only when an Admin starts it", () => {
    expect(scheduleWords("")).toBe("Only when an Admin runs it");
  });
});
