import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { SettingKind, SettingSchema } from "@/gen/nama/v1/plugin_pb";
import { ChoiceInput } from "@/shared/components/setting-input/choice-input";
import { SetSecret } from "@/shared/components/setting-input/set-secret";
import { forgottenField } from "@/shared/lib/setting-values";

afterEach(cleanup);

// A row's first/last-row styles go by `:first-child` and `:last-child`, so a hidden input beside it
// would take that place (#2127).
describe("a hidden input stays inside its setting's row", () => {
  test("a forgotten secret keeps its key inside its row, the first and only child", () => {
    const setting = create(SettingSchema, {
      key: "api_key",
      label: "API key",
      kind: SettingKind.SECRET,
    });
    const { container, getByRole } = render(
      <SetSecret setting={setting} error={undefined} forgettable />,
    );
    fireEvent.click(getByRole("button", { name: "Forget API key" }));
    // The X is gone once it asks, so this is the confirmation's Forget.
    fireEvent.click(getByRole("button", { name: "Forget API key" }));

    expect(
      [...container.children].map((row) =>
        row.querySelector(`input[name="${forgottenField}"]`)?.getAttribute("value"),
      ),
    ).toStrictEqual(["api_key"]);
  });

  test("a locked choice keeps its stored choice inside its row, the last and only child", () => {
    const setting = create(SettingSchema, {
      key: "profile",
      label: "Profile",
      kind: SettingKind.CHOICE,
      value: { value: { case: "textValue", value: "hd" } },
    });
    const { container } = render(
      <ChoiceInput setting={setting} choices={undefined} error={undefined} required={false} />,
    );

    expect(
      [...container.children].map((row) =>
        row.querySelector('input[type="hidden"][name="profile"]')?.getAttribute("value"),
      ),
    ).toStrictEqual(["hd"]);
  });
});
