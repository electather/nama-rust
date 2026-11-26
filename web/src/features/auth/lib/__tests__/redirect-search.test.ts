import { expect, test } from "vite-plus/test";

import { redirectSearch } from "@/features/auth/lib/redirect-search";

test("redirectSearch keeps a path on this Server", () => {
  expect(redirectSearch({ redirect: "/library/1?sort=title#top" })).toStrictEqual({
    redirect: "/library/1?sort=title#top",
  });
});

test("redirectSearch overwrites a redirect elsewhere with undefined", () => {
  expect(redirectSearch({ redirect: "https://evil.example/" })).toStrictEqual({
    redirect: undefined,
  });
});

test("redirectSearch overwrites a protocol-relative redirect with undefined", () => {
  expect(redirectSearch({ redirect: "//evil.example/" })).toStrictEqual({ redirect: undefined });
});
