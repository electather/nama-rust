import { create } from "@bufbuild/protobuf";
import { createRouterTransport } from "@connectrpc/connect";
import { expect, test } from "vite-plus/test";

import { frameSheets } from "@/features/player/lib/preview-frame";
import { PlaybackPlanSchema, PreviewFramesSchema } from "@/gen/nama/v1/playback_pb";
import { serverTransport } from "@/shared/lib/server";

const page = createRouterTransport(() => {});
const previewFrames = create(PreviewFramesSchema, { count: 3 });
const pathFor = (url: string, transport = page) =>
  frameSheets(create(PlaybackPlanSchema, { url, previewFrames }), 7n, transport)?.path;

test("a signed plan's Preview frames load under its signed prefix", () => {
  expect(pathFor("/media/signed/g.1/sig/versions/7/stream")).toBe(
    "/media/signed/g.1/sig/versions/7/preview-frames",
  );
});

test("an unsigned plan's Preview frames load from the Version's own path", () => {
  for (const url of ["/media/versions/7/stream", "/media/plans/3/index.m3u8", ""]) {
    expect(pathFor(url)).toBe("/media/versions/7/preview-frames");
  }
});

test("at the Hub, Preview frames load from the Server", () => {
  const friend = serverTransport("https://friend.example", []);
  expect(pathFor("/media/signed/g.1/sig/plans/3/index.m3u8", friend)).toBe(
    "https://friend.example/media/signed/g.1/sig/versions/7/preview-frames",
  );
  expect(pathFor("https://friend.example/media/signed/g.1/sig/plans/3/index.m3u8", friend)).toBe(
    "https://friend.example/media/signed/g.1/sig/versions/7/preview-frames",
  );
  expect(pathFor("", friend)).toBe("https://friend.example/media/versions/7/preview-frames");
});

test("a plan without Preview frames has no sheets", () => {
  expect(
    frameSheets(create(PlaybackPlanSchema, { url: "/media/versions/7/stream" }), 7n, page),
  ).toBeUndefined();
});
