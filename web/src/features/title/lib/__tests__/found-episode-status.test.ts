import { create } from "@bufbuild/protobuf";
import { afterEach, beforeEach, describe, expect, test, vi } from "vite-plus/test";

import { foundEpisodeStatus } from "@/features/title/lib/found-episode-status";
import { FoundEpisodeSchema, RequestSchema, RequestState } from "@/gen/nama/v1/request_pb";
import { type TitleFacts, titleStanding } from "@/shared/lib/request-standing";
import { calendarDay } from "@/shared/lib/title-facts";

/** Episode 5 of Season 1, missing from the Server, standing as `facts` say. */
const missing = (facts: TitleFacts) => ({
  kind: "missing" as const,
  seasonNumber: 1,
  episodeNumber: 5,
  episode: create(FoundEpisodeSchema, { episodeNumber: 5, title: "The Banks Must Pay" }),
  standing: titleStanding(facts),
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
    const item = missing({ request: undefined, inLibrary: false, airDate: "2026-12-03" });
    expect(foundEpisodeStatus(item)).toBe(`Airs ${calendarDay("2026-12-03")}`);
  });

  test("says its Season's Request's state while it has one", () => {
    const request = create(RequestSchema, { state: RequestState.PENDING });
    expect(foundEpisodeStatus(missing({ request, inLibrary: false }))).toBe("Pending approval");
  });

  test("says it isn't on the Server while nobody asked", () => {
    expect(foundEpisodeStatus(missing({ request: undefined, inLibrary: false }))).toBe(
      "Not on this Server",
    );
  });
});
