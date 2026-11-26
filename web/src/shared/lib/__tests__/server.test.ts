import { createRouterTransport } from "@connectrpc/connect";
import { expect, test } from "vite-plus/test";

import { onServer, serverTransport } from "@/shared/lib/server";

test("a path is on the Server its transport goes to, or on the page's own when it names none", () => {
  expect(onServer(serverTransport("https://friend.example", []), "/custom-images/movie/1")).toBe(
    "https://friend.example/custom-images/movie/1",
  );
  expect(onServer(serverTransport("", []), "/custom-images/movie/1")).toBe(
    "/custom-images/movie/1",
  );
  expect(
    onServer(
      createRouterTransport(() => {}),
      "/custom-images/movie/1",
    ),
  ).toBe("/custom-images/movie/1");
});

test("a URL that isn't a path on the Server stays as it is", () => {
  const friend = serverTransport("https://friend.example", []);
  for (const url of ["", "https://cdn.example/x.jpg", "//cdn.example/x.jpg", "data:,x"]) {
    expect(onServer(friend, url)).toBe(url);
  }
});
