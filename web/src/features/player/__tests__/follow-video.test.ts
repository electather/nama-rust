import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { followVideo, REPORT_EVERY_MS } from "@/features/player/lib/follow-video";

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

type Report = Parameters<typeof followVideo>[2];

/** A `<video>` whose file's length is known, so it has a position to report. */
function loadedVideo() {
  const video = document.createElement("video");
  Object.defineProperty(video, "duration", { value: 7000 });
  return video;
}

test("a paused video reports again every few seconds until it plays again", () => {
  const video = loadedVideo();
  const report = vi.fn<Report>();
  followVideo(video, 0n, report);

  video.dispatchEvent(new Event("pause"));
  vi.advanceTimersByTime(REPORT_EVERY_MS * 2);
  expect(report).toHaveBeenCalledTimes(3);

  video.dispatchEvent(new Event("play"));
  vi.advanceTimersByTime(REPORT_EVERY_MS * 2);
  expect(report).toHaveBeenCalledTimes(3);
});

test("a paused video stops reporting once it's no longer followed", () => {
  const video = loadedVideo();
  const report = vi.fn<Report>();
  const unfollow = followVideo(video, 0n, report);
  video.dispatchEvent(new Event("pause"));

  unfollow();
  vi.advanceTimersByTime(REPORT_EVERY_MS * 2);

  expect(report.mock.calls.map(([, final]) => final)).toStrictEqual([false, true]);
});
