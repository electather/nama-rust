import { renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { useShare } from "@/shared/hooks/use-share";

const link = { url: "https://youtu.be/WBlIUsFEnsw", title: "Official Trailer" };

afterEach(() => {
  vi.unstubAllGlobals();
});

test("a link goes to the share sheet where there is one, and is copied where there isn't", async () => {
  const share = vi.fn<(data: ShareData) => Promise<void>>(async () => {});
  const writeText = vi.fn<(text: string) => Promise<void>>(async () => {});
  vi.stubGlobal("navigator", { share, clipboard: { writeText } });
  const { result } = renderHook(() => useShare());

  await expect(result.current(link)).resolves.toBe("shared");
  expect(share).toHaveBeenCalledWith(link);

  vi.stubGlobal("navigator", { clipboard: { writeText } });
  await expect(result.current(link)).resolves.toBe("copied");
  expect(writeText).toHaveBeenCalledWith(link.url);
});
