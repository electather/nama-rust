import { create } from "@bufbuild/protobuf";
import { afterAll, beforeAll, describe, expect, test, vi } from "vite-plus/test";

import { BackupSchema } from "@/gen/nama/v1/server_pb";

const backupOf = (bytes: number) => create(BackupSchema, { sizeBytes: BigInt(bytes) });

describe("a Backup's size", () => {
  beforeAll(() => {
    // A browser in German writes "15,2 MB"; a formatter left on the browser's locale would follow it.
    const { NumberFormat } = Intl;
    vi.spyOn(Intl, "NumberFormat").mockImplementation(
      class extends NumberFormat {
        public constructor(locales?: Intl.LocalesArgument, options?: Intl.NumberFormatOptions) {
          super(locales ?? "de", options);
        }
      },
    );
    vi.resetModules();
  });

  afterAll(() => {
    vi.restoreAllMocks();
  });

  test("the browser here writes numbers in German", () => {
    expect(new Intl.NumberFormat().format(1.5)).toBe("1,5");
  });

  test.each([
    [40_000, "40 kB"],
    [15_200_000, "15.2 MB"],
    [999_950_000, "1 GB"],
    [2_500_000_000, "2.5 GB"],
  ])("%i bytes reads %s", async (bytes, shown) => {
    // Imported after the stub, so its formatters are built under the German default.
    const { backupSize } = await import("@/features/admin-backups/lib/backup-facts");
    expect(backupSize(backupOf(bytes))).toBe(shown);
  });
});
