import { expect, test } from "vite-plus/test";

import { trailerPlayer } from "@/features/title/lib/trailer-player";

test("a YouTube or Vimeo page gives its host's player, and anything else nothing", () => {
  expect(
    [
      "https://www.youtube.com/watch?v=WBlIUsFEnsw",
      "https://youtu.be/WBlIUsFEnsw",
      "https://vimeo.com/123456789",
      "https://www.youtube.com/watch",
      "https://example.com/watch?v=WBlIUsFEnsw",
      "not a url",
    ].map((url) => trailerPlayer(url)),
  ).toStrictEqual([
    "https://www.youtube-nocookie.com/embed/WBlIUsFEnsw?autoplay=1",
    "https://www.youtube-nocookie.com/embed/WBlIUsFEnsw?autoplay=1",
    "https://player.vimeo.com/video/123456789?autoplay=1&dnt=1",
    undefined,
    undefined,
    undefined,
  ]);
});
