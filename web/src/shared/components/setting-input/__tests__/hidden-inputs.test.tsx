import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { SettingKind, SettingSchema } from "@/gen/nama/v1/plugin_pb";
import { ChoiceInput } from "@/shared/components/setting-input/choice-input";
import { SetSecret } from "@/shared/components/setting-input/set-secret";
import { forgottenField } from "@/shared/lib/setting-values";

afterEach(cleanup);

/** The element `container` renders at its top whose descendant `selector` finds. */
function rowHolding(container: HTMLElement, selector: string) {
  return [...container.children].find((row) => row.querySelector(selector) !== null);
}

// A row's first/last-row styles go by `:first-child` and `:last-child`, so a hidden input beside it
// would take that place (#2127).
describe("a hidden input stays inside its setting's row", () => {
  test("a forgotten secret's row holds its key and stays its parent's first child", () => {
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
    const key = `input[name="${forgottenField}"]`;
    const row = rowHolding(container, key);

    expect(row?.matches(":first-child")).toBe(true);
    expect(row?.querySelector(key)?.getAttribute("value")).toBe("api_key");
  });

  test("a locked choice's row holds its stored choice and stays its parent's last child", () => {
    const setting = create(SettingSchema, {
      key: "profile",
      label: "Profile",
      kind: SettingKind.CHOICE,
      value: { value: { case: "textValue", value: "hd" } },
    });
    const { container } = render(
      <ChoiceInput setting={setting} choices={undefined} error={undefined} required={false} />,
    );
    const stored = 'input[type="hidden"][name="profile"]';
    const row = rowHolding(container, stored);

    expect(row?.matches(":last-child")).toBe(true);
    expect(row?.querySelector(stored)?.getAttribute("value")).toBe("hd");
  });
});
