import { create } from "@bufbuild/protobuf";
import { afterEach, beforeEach, describe, expect, test, vi } from "vite-plus/test";

import { foundEpisodeStatus } from "@/features/title/lib/found-episode-label";
import type { FoundRowEpisode } from "@/features/title/lib/show-row";
import { FoundEpisodeSchema } from "@/gen/nama/v1/request_pb";
import { calendarDay } from "@/shared/lib/title-facts";

const missing = (airDate?: string): FoundRowEpisode => ({
  kind: "missing",
  seasonNumber: 1,
  episodeNumber: 5,
  episode: create(FoundEpisodeSchema, { episodeNumber: 5, title: "The Banks Must Pay", airDate }),
  request: undefined,
  askable: false,
});

describe("a missing Episode's status", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-11-27T12:00:00"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  test("says when it airs while it hasn't yet", () => {
    expect(foundEpisodeStatus(missing("2026-12-03"))).toBe(`Airs ${calendarDay("2026-12-03")}`);
  });

  test("says it isn't on the Server once it aired", () => {
    expect(foundEpisodeStatus(missing("2026-11-26"))).toBe("Not on this Server");
  });

  test("says it isn't on the Server on the day it airs", () => {
    expect(foundEpisodeStatus(missing("2026-11-27"))).toBe("Not on this Server");
  });

  test("says it isn't on the Server without an air date", () => {
    expect(foundEpisodeStatus(missing())).toBe("Not on this Server");
  });
});
