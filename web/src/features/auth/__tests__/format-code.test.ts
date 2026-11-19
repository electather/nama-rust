import { expect, test } from "vite-plus/test";

import { browserName } from "@/features/auth/lib/browser-name";
import { formatCode } from "@/features/auth/lib/format-code";

test("a typed code is upper-cased, de-look-aliked, cut and grouped", () => {
  expect(formatCode("4hg7 pxzm-k9qo", 12)).toBe("4HG7-PXZM-K9Q0");
  expect(formatCode("il-abcdefghij", 8)).toBe("11AB-CDEF");
  expect(formatCode("ab", 8)).toBe("AB");
  expect(formatCode("--", 8)).toBe("");
});

test("a browser is named by its browser and system", () => {
  const mac = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36";
  expect(browserName(`${mac} (KHTML, like Gecko) Chrome/140.0 Safari/537.36`)).toBe(
    "Chrome on Mac",
  );
  expect(browserName(`${mac} Version/26.0 Safari/605.1.15`)).toBe("Safari on Mac");
  expect(
    browserName("Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X) Version/26.0 Safari/604.1"),
  ).toBe("Safari on iPhone");
  expect(
    browserName("Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0"),
  ).toBe("Firefox on Linux");
  expect(browserName("curl/8.0")).toBe("");
});
