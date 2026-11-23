import { describe, expect, test } from "vite-plus/test";

import { cronOf, pickOf, withKind } from "@/features/admin-jobs/lib/schedule-picks";

describe("a schedule in the picker", () => {
  test.each([
    ["*/15 * * * *", "minutes"],
    ["0 * * * *", "hours"],
    ["30 */6 * * *", "hours"],
    ["10 4 * * *", "daily"],
    ["30 2 * * 1-5", "weekly"],
    ["0 9 * * 1,3,5", "weekly"],
    ["0 3 28 * *", "monthly"],
  ])("%s is picked as %s and comes back as written", (cron, kind) => {
    const pick = pickOf(cron);
    expect(pick.kind).toBe(kind);
    expect(cronOf(pick)).toBe(cron);
  });

  test.each([
    "*/7 * * * *",
    "7 * * * *",
    "0 4 29 * *",
    "0 4 * 1 *",
    "0 4 * * mon",
    "0 25 * * *",
    "0 4 * *",
  ])("%s has no shape the picker shows, so it's Custom as typed", (cron) => {
    expect(pickOf(cron)).toMatchObject({ kind: "custom", custom: cron });
  });

  test("weekdays picked out of order, Sunday as 7 included, are written in order as ranges", () => {
    expect(pickOf("0 6 * * 7,1-3").days).toStrictEqual([0, 1, 2, 3]);
    expect(cronOf({ ...pickOf("0 6 * * 1"), days: [5, 0, 1, 2, 3, 4] })).toBe("0 6 * * 0-5");
    expect(cronOf({ ...pickOf("0 6 * * 1"), days: [6, 0, 1, 3] })).toBe("0 6 * * 0,1,3,6");
  });

  test("switching to Custom starts from what was picked", () => {
    expect(withKind(pickOf("*/10 * * * *"), "custom").custom).toBe("*/10 * * * *");
  });
});
