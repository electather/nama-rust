import { expect, test, vi } from "vite-plus/test";

import {
  firstFramePlayed,
  playerOpened,
  recordDiagnosticsIfExported,
  stallEnded,
  stallStarted,
} from "@/shared/lib/diagnostics";

const { recordSpan } = vi.hoisted(() => ({ recordSpan: vi.fn<(name: string) => void>() }));
// oxlint-disable-next-line vitest/prefer-import-in-mock -- the fake has only what these spans call, so it can't be typed as the module.
vi.mock("@/shared/lib/diagnostics-recorder", () => ({
  recorderAt: () => ({ recordSpan, recordError: vi.fn<() => void>() }),
}));

test("a wait is a playback stall only after the first frame, and never a seek's own", async () => {
  await recordDiagnosticsIfExported(true);
  playerOpened();
  stallStarted({ seeking: false });
  firstFramePlayed();
  stallEnded();
  stallStarted({ seeking: true });
  stallEnded();
  stallStarted({ seeking: false });
  stallEnded();
  expect(recordSpan.mock.calls.map(([name]) => name)).toStrictEqual([
    "playback start",
    "playback stall",
  ]);
});
