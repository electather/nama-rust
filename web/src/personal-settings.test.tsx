import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { SettingKind } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { UpdatePersonalSettingsRequest } from "@/gen/nama/v1/user_pb";

import { fakeServer, newUser, openApp } from "./test-app";
import { expectUnderField, refused } from "./test-refusal";

afterEach(cleanup);

/** ntfy's Personal settings: a topic set to `alerts`, a set access token and a bool. */
const ntfy = {
  pluginId: "ntfy",
  name: "ntfy",
  settings: [
    {
      key: "topic",
      label: "Topic",
      description: "",
      kind: SettingKind.TEXT,
      required: true,
      value: { value: { case: "textValue" as const, value: "alerts" } },
    },
    {
      key: "token",
      label: "Access token",
      description: "",
      kind: SettingKind.SECRET,
      secretSet: true,
    },
    { key: "priority", label: "High priority", description: "", kind: SettingKind.BOOL },
  ],
};

/** Opens Settings with ntfy's Personal settings, saving them with `update`. */
function openWithNtfy(update: (request: UpdatePersonalSettingsRequest) => object = () => ({})) {
  openApp(
    "/settings",
    fakeServer(
      { signedIn: newUser },
      {},
      { listPersonalSettings: () => ({ plugins: [ntfy] }), updatePersonalSettings: update },
    ),
  );
}

const card = async () => within(await screen.findByRole("listitem", { name: "ntfy" }));

test("Settings shows no Personal settings while no Plugin declares any", async () => {
  let listed = false;
  openApp(
    "/settings",
    fakeServer(
      { signedIn: newUser },
      {},
      {
        listPersonalSettings: () => {
          listed = true;
          return {};
        },
      },
    ),
  );

  await screen.findByRole("heading", { name: "Devices" });
  await waitFor(() => {
    expect(listed).toBe(true);
  });
  expect(screen.queryByRole("heading", { name: "Personal settings" })).toBeNull();
});

test("a required Personal setting is required", async () => {
  openWithNtfy();

  expect((await card()).getByLabelText<HTMLInputElement>("Topic").required).toBe(true);
});

test("a set secret is said to be set, never filled in", async () => {
  openWithNtfy();

  const ntfyCard = await card();
  expect(ntfyCard.getByText("Set")).toBeDefined();
  expect(ntfyCard.queryByLabelText("Access token")).toBeNull();
  fireEvent.click(ntfyCard.getByRole("button", { name: "Replace Access token" }));
  const token = ntfyCard.getByLabelText<HTMLInputElement>("Access token");
  expect([token.type, token.value]).toEqual(["password", ""]);
});

test("saving sends the values typed, keeping a set secret by leaving it out", async () => {
  const sent = new Promise<UpdatePersonalSettingsRequest>((resolve) => {
    openWithNtfy((request) => {
      resolve(request);
      return {};
    });
  });
  const ntfyCard = await card();
  fireEvent.change(ntfyCard.getByLabelText("Topic"), { target: { value: "movies" } });
  fireEvent.click(ntfyCard.getByLabelText("High priority"));

  fireEvent.click(ntfyCard.getByRole("button", { name: "Save" }));

  const { pluginId, settings } = await sent;
  expect(pluginId).toBe("ntfy");
  expect(Object.keys(settings)).toEqual(["topic", "priority"]);
  expect(settings["topic"]?.value).toEqual({ case: "textValue", value: "movies" });
  expect(settings["priority"]?.value).toEqual({ case: "boolValue", value: true });
  expect(await screen.findByText("Saved.")).toBeDefined();
});

test("a refused setting shows its refusal under its field", async () => {
  openWithNtfy(() => {
    throw refused(Code.InvalidArgument, "topic is required", {
      reason: RefusalReason.FIELD_REQUIRED,
      field: "settings.topic",
    });
  });

  fireEvent.click((await card()).getByRole("button", { name: "Save" }));

  await expectUnderField("Topic", "This setting is required.");
});
