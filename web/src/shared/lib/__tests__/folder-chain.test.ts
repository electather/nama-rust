import { describe, expect, test } from "vite-plus/test";

import { folderChain } from "@/shared/lib/folder-chain";

describe("the folders down to a path", () => {
  test.each([
    ["/media/films/", ["/media", "/media/films"]],
    ["/", []],
    ["", []],
    ["media/films", []],
    [String.raw`c:\Media\Films`, ["C:\\", String.raw`C:\Media`, String.raw`C:\Media\Films`]],
    ["D:", ["D:\\"]],
  ] as const)("of %j are %j", (path, chain) => {
    expect(folderChain(path)).toStrictEqual(chain);
  });
});
