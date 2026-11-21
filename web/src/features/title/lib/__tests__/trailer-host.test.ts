import { expect, test } from "vite-plus/test";

import { trailerHost } from "@/features/title/lib/trailer-player";

test("a Trailer's host is named for YouTube and Vimeo, and nothing else", () => {
  expect(
    [
      "https://m.youtube.com/watch?v=WBlIUsFEnsw",
      "https://youtu.be/WBlIUsFEnsw",
      "https://vimeo.com/123456789",
      "https://example.com/trailer",
    ].map((url) => trailerHost(url)),
  ).toStrictEqual(["YouTube", "YouTube", "Vimeo", undefined]);
});
