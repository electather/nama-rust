import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { SettingRow } from "@/shared/components/setting-row";

afterEach(cleanup);

// happy-dom applies no CSS, so this checks each row carries the class and which row `last:` picks.
test("every row carries the last-row padding cancel, and only the last row matches it", () => {
  const { container } = render(
    <div>
      <SettingRow label="First">{(control) => <input {...control} />}</SettingRow>
      <SettingRow label="Last">{(control) => <input {...control} />}</SettingRow>
    </div>,
  );
  const rows = [...container.querySelectorAll(":scope > div > div")];
  expect(
    rows.map((row) => [row.classList.contains("last:-mb-3.5"), row.matches(":last-child")]),
  ).toStrictEqual([
    [true, false],
    [true, true],
  ]);
});
