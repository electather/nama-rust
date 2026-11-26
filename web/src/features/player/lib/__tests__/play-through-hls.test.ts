import { M3U8Parser, PlaylistLevelType } from "hls.js";
import { expect, test } from "vite-plus/test";

test("at the Hub, hls.js loads a plan's segments from the Server, under its signed prefix", () => {
  const plan = "https://friend.example/media/signed/grant/signature/plans/0/index.m3u8";
  // As the Server writes a plan's playlist (`playback/running/timeline.rs`).
  const playlist = [
    "#EXTM3U",
    "#EXT-X-VERSION:7",
    "#EXT-X-TARGETDURATION:6",
    "#EXT-X-PLAYLIST-TYPE:VOD",
    '#EXT-X-MAP:URI="init.mp4"',
    "#EXTINF:6.000,",
    "0.m4s",
    "#EXT-X-ENDLIST",
  ].join("\n");

  const [segment] = M3U8Parser.parseLevelPlaylist(
    playlist,
    plan,
    0,
    PlaylistLevelType.MAIN,
    0,
    null,
  ).fragments;

  expect([segment?.initSegment?.url, segment?.url]).toStrictEqual([
    "https://friend.example/media/signed/grant/signature/plans/0/init.mp4",
    "https://friend.example/media/signed/grant/signature/plans/0/0.m4s",
  ]);
});
