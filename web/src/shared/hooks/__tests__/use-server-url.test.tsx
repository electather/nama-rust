import { renderHook } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { useServerUrl } from "@/shared/hooks/use-server-url";
import { useSrcSet } from "@/shared/hooks/use-src-set";
import { onServerAt } from "@/test-server-at";

const poster = "/media/artwork/tmdb/%2Fposter.jpg";

/** `useServerUrl(url)` on the Server at `origin`. */
function serverUrl(url?: string, origin?: string) {
  return renderHook(() => useServerUrl(url), { wrapper: onServerAt(origin) }).result.current;
}

test("a path the Server sent is on the Server the page talks to", () => {
  expect(serverUrl(poster)).toBe(`https://friend.example${poster}`);
});

test("on the Server's own page a path stays as it is", () => {
  expect(serverUrl(poster, "")).toBe(poster);
});

test("an absolute, protocol-relative, empty or missing URL stays as it is", () => {
  expect(serverUrl("https://image.tmdb.org/t/p/w500/x.jpg")).toBe(
    "https://image.tmdb.org/t/p/w500/x.jpg",
  );
  expect(serverUrl("data:image/png;base64,AAAA")).toBe("data:image/png;base64,AAAA");
  expect(serverUrl("//cdn.example/x.jpg")).toBe("//cdn.example/x.jpg");
  expect(serverUrl("")).toBe("");
  expect(serverUrl()).toBeUndefined();
});

test("a srcset offers each width on the Server the page talks to", async () => {
  const { result } = renderHook(() => useSrcSet(poster), { wrapper: onServerAt() });

  await expect
    .poll(() => result.current)
    .toBe(
      `https://friend.example${poster}?width=320 320w, https://friend.example${poster}?width=640 640w`,
    );
});
