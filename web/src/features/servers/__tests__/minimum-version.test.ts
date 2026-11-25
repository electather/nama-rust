import { expect, test } from "vite-plus/test";

import { isBelowMinimumVersion } from "@/features/servers/lib/minimum-version";

test("versions before 0.2.1, or unreadable, are below the minimum, and later ones aren't", () => {
  const below = ["0.2.0", "0.1.9", "0.2", "", "dev"].filter((version) =>
    isBelowMinimumVersion(version),
  );
  const atOrAbove = ["0.2.1", "0.2.1-rc.1", "0.10.0", "1.0.0"].filter((version) =>
    isBelowMinimumVersion(version),
  );

  expect(below).toHaveLength(5);
  expect(atOrAbove).toStrictEqual([]);
});
