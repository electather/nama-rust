import { expect, test, vi } from "vite-plus/test";

import {
  firstFramePlayed,
  playerOpened,
  recordDiagnosticsIfExported,
  stallEnded,
  stallStarted,
} from "@/shared/lib/diagnostics";
import { recordSpan } from "@/shared/lib/diagnostics-recorder";

// oxlint-disable-next-line vitest/prefer-import-in-mock -- the fake has only what these spans call, so it can't be typed as the module.
vi.mock("@/shared/lib/diagnostics-recorder", () => ({
  recordSpan: vi.fn<() => void>(),
  recordError: vi.fn<() => void>(),
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
  expect(vi.mocked(recordSpan).mock.calls.map(([name]) => name)).toStrictEqual([
    "playback start",
    "playback stall",
  ]);
});
