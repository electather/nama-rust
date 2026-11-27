import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { SettingKind, SettingSchema } from "@/gen/nama/v1/plugin_pb";
import { SettingInput } from "@/shared/components/setting-input/setting-input";
import { dropdown } from "@/test-settings";

const LANGUAGES = [
  { value: "en-US", label: "English (United States)" },
  { value: "en-GB", label: "English (United Kingdom)" },
  { value: "de-DE", label: "German (Germany)" },
  { value: "de-AT", label: "German (Austria)" },
  { value: "fr-FR", label: "French (France)" },
  { value: "es-ES", label: "Spanish (Spain)" },
  { value: "it-IT", label: "Italian (Italy)" },
  { value: "ja-JP", label: "Japanese (Japan)" },
  { value: "ko-KR", label: "Korean (South Korea)" },
  { value: "nl-NL", label: "Dutch (Netherlands)" },
  { value: "sv-SE", label: "Swedish (Sweden)" },
];

/** Renders a Language choice setting listing `choices`, holding en-US, from a Plugin with a check. */
function renderLanguage(choices: typeof LANGUAGES) {
  const setting = create(SettingSchema, {
    key: "language",
    label: "Language",
    kind: SettingKind.CHOICE,
    choices,
    value: { value: { case: "textValue", value: "en-US" } },
  });
  return render(
    <form>
      <SettingInput setting={setting} checksSettings error={undefined} checksRequired />
    </form>,
  );
}

/** The options the open list shows once `typed` is searched for. */
function searched(typed: string) {
  fireEvent.click(dropdown("Language"));
  fireEvent.change(screen.getByRole("combobox", { name: "Search" }), {
    target: { value: typed },
  });
  return screen.queryAllByRole("option").map((option) => option.textContent);
}

afterEach(cleanup);

describe("a choice setting listing its own choices", () => {
  test("holding nothing starts on its default", () => {
    render(
      <SettingInput
        setting={create(SettingSchema, {
          key: "language",
          label: "Language",
          kind: SettingKind.CHOICE,
          choices: LANGUAGES,
          defaultValue: { value: { case: "textValue", value: "de-DE" } },
        })}
        error={undefined}
        checksRequired
      />,
    );

    expect(dropdown("Language").textContent).toBe("German (Germany)");
  });

  test("can be picked before any test", () => {
    renderLanguage(LANGUAGES.slice(0, 3));

    expect(dropdown("Language").hasAttribute("disabled")).toBe(false);
    expect(dropdown("Language").textContent).toBe("English (United States)");
  });

  test("past 10 choices sends what's chosen as a form value", () => {
    const { container } = renderLanguage(LANGUAGES);

    expect(dropdown("Language").hasAttribute("disabled")).toBe(false);
    expect(dropdown("Language").textContent).toBe("English (United States)");
    expect(container.querySelector('input[name="language"]')?.getAttribute("value")).toBe("en-US");
  });

  test("past 10 choices narrows them to those whose value has what's typed", () => {
    renderLanguage(LANGUAGES);

    expect(searched("de-")).toStrictEqual(["German (Germany)", "German (Austria)"]);
  });

  test("past 10 choices narrows them to those whose label has what's typed", () => {
    renderLanguage(LANGUAGES);

    expect(searched("swed")).toStrictEqual(["Swedish (Sweden)"]);
  });
});
