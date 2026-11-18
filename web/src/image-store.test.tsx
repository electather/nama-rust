import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, type ServiceImpl } from "@connectrpc/connect";
import { fireEvent, screen, within } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { ExtensionPoint, PluginKind, PluginService } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { GetImageSettingsResponseSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, newUser, openApp } from "./test-app";
import { refused } from "./test-refusal";

const manageSettings = [...newUser, Permission.MANAGE_SETTINGS];

type ImageSettingsAnswer = MessageInitShape<typeof GetImageSettingsResponseSchema>;

const plugin = (id: string, name: string, enabled: boolean, extensionPoints: ExtensionPoint[]) => ({
  id,
  name,
  version: "0.1.0",
  kind: PluginKind.INSTALLED,
  enabled,
  extensionPoints,
});

/**
 * An enabled S3 store, a store Plugin that is disabled, and TMDB, which keeps no images, so only S3
 * may keep them.
 */
const summaries = [
  plugin("s3", "S3", true, [ExtensionPoint.BLOB_STORE]),
  plugin("old-store", "Old store", false, [ExtensionPoint.BLOB_STORE]),
  plugin("tmdb", "TMDB", true, [ExtensionPoint.TITLE_SOURCE]),
];

/** Opens Settings for a User who manages settings, on a fake Server answering with `server`. */
function openSettings(server: Partial<ServiceImpl<typeof ServerService>>) {
  openApp(
    "/settings",
    fakeServer({ signedIn: manageSettings, serverService: server }, {}, {}, (router) => {
      router.service(PluginService, { listPlugins: () => ({ summaries }) });
    }),
  );
}

/** The image store's part of Settings, once it shows. */
async function imageStore() {
  return within(await screen.findByRole("region", { name: "Image store" }));
}

/** Picks the store `pluginId` in `store`'s picker and asks to switch to it. */
function switchTo(store: Awaited<ReturnType<typeof imageStore>>, pluginId: string) {
  fireEvent.change(store.getByLabelText("Keep images on"), { target: { value: pluginId } });
  fireEvent.click(store.getByRole("button", { name: "Switch image store" }));
}

test("with local disk in use, the picker chooses it and offers only the enabled Blob store Plugins", async () => {
  openSettings({});

  const store = await imageStore();
  expect(store.getByText("Local disk", { selector: "span" })).toBeDefined();
  await store.findByRole("option", { name: "S3" });

  const options = store.getAllByRole("option").map((option) => option.textContent);
  expect(options).toEqual(["Local disk", "S3"]);
  expect(store.getByLabelText<HTMLSelectElement>("Keep images on").value).toBe("");
});

test("switching the image store shows the copy's progress, then the new store", async () => {
  const switches: string[] = [];
  // What the next look at the image settings answers, and the one after it, and on.
  const answers: { now: ImageSettingsAnswer; next: ImageSettingsAnswer } = {
    now: { settings: {} },
    next: { settings: {} },
  };
  openSettings({
    getImageSettings: () => {
      const answer = answers.now;
      answers.now = answers.next;
      return answer;
    },
    switchImageStore: ({ pluginId }) => {
      switches.push(pluginId);
      answers.now = { settings: {}, switching: { toPluginId: pluginId, copied: 1, total: 3 } };
      answers.next = { settings: { storePluginId: pluginId } };
      return { switching: { toPluginId: pluginId } };
    },
  });

  const store = await imageStore();
  await store.findByRole("option", { name: "S3" });
  switchTo(store, "s3");

  await expect(store.findByText("Copied 1 of 3 images to S3…")).resolves.toBeDefined();
  await expect(store.findByText("S3", { selector: "span" })).resolves.toBeDefined();
  expect(store.queryByText(/Copied/u)).toBeNull();
  expect(switches).toEqual(["s3"]);
});

test("switching back to local disk names no Plugin", async () => {
  const switches: string[] = [];
  openSettings({
    getImageSettings: () => ({ settings: { storePluginId: "s3" } }),
    switchImageStore: ({ pluginId }) => {
      switches.push(pluginId);
      return {};
    },
  });

  const store = await imageStore();
  await store.findByText("S3", { selector: "span" });
  await store.findByRole("option", { name: "S3" });
  switchTo(store, "");

  await expect.poll(() => switches).toEqual([""]);
});

test("an enabled Blob store Plugin in use is chosen in the picker, adding no option", async () => {
  openSettings({ getImageSettings: () => ({ settings: { storePluginId: "s3" } }) });

  const store = await imageStore();
  await store.findByRole("option", { name: "S3" });

  const picker = store.getByLabelText<HTMLSelectElement>("Keep images on");
  expect(picker.value).toBe("s3");
  expect(picker.selectedOptions[0]?.textContent).toBe("S3");
  const options = [...picker.options].map((option) => option.textContent);
  expect(options).toEqual(["Local disk", "S3"]);
});

test("a store that fails its check is refused with why", async () => {
  openSettings({
    switchImageStore: () => {
      throw refused(Code.FailedPrecondition, "the new image store failed its check", {
        reason: RefusalReason.IMAGE_STORE_FAILED_CHECK,
        field: "plugin_id",
        params: { pluginId: "s3", reason: "couldn't write to it: access denied" },
      });
    },
  });

  const store = await imageStore();
  await store.findByRole("option", { name: "S3" });
  switchTo(store, "s3");

  await expect(
    store.findByText(
      "S3 failed its check, so the images stay where they are: couldn't write to it: access denied.",
    ),
  ).resolves.toBeDefined();
  expect(store.getByText("Local disk", { selector: "span" })).toBeDefined();
});

test("a switch that stopped shows why, with the old store still in use", async () => {
  openSettings({
    getImageSettings: () => ({
      settings: {},
      switching: {
        toPluginId: "s3",
        copied: 1,
        total: 3,
        failure: "couldn't copy a.jpg: timed out",
      },
    }),
  });

  const store = await imageStore();

  await expect(
    store.findByText(
      /Moving the images to .+ stopped, so they stay on Local disk: couldn't copy a\.jpg: timed out\./u,
    ),
  ).resolves.toBeDefined();
});

test("a disabled Blob store Plugin in use shows its Plugin name, chosen but not offered", async () => {
  openSettings({ getImageSettings: () => ({ settings: { storePluginId: "old-store" } }) });

  const store = await imageStore();
  await expect(store.findByText("Old store", { selector: "span" })).resolves.toBeDefined();
  await store.findByRole("option", { name: "S3" });
  const picker = store.getByLabelText<HTMLSelectElement>("Keep images on");
  expect(picker.value).toBe("old-store");
  expect(picker.selectedOptions[0]?.textContent).toBe("Old store (disabled)");
  expect(picker.selectedOptions[0]?.disabled).toBe(true);
  const offered = [...picker.options]
    .filter((option) => !option.disabled)
    .map((option) => option.textContent);
  expect(offered).toEqual(["Local disk", "S3"]);
});

test("a stopped switch to a disabled Blob store Plugin names it, with one full stop", async () => {
  openSettings({
    getImageSettings: () => ({
      settings: {},
      switching: { toPluginId: "old-store", copied: 1, total: 3, failure: "it went away." },
    }),
  });

  const store = await imageStore();

  await expect(
    store.findByText(
      "Moving the images to Old store stopped, so they stay on Local disk: it went away.",
    ),
  ).resolves.toBeDefined();
});

test("a failed check by a disabled Blob store Plugin names it, with one full stop", async () => {
  openSettings({
    switchImageStore: () => {
      throw refused(Code.FailedPrecondition, "the new image store failed its check", {
        reason: RefusalReason.IMAGE_STORE_FAILED_CHECK,
        field: "plugin_id",
        params: { pluginId: "old-store", reason: "Access denied." },
      });
    },
  });

  const store = await imageStore();
  await store.findByRole("option", { name: "S3" });
  switchTo(store, "s3");

  await expect(
    store.findByText(
      "Old store failed its check, so the images stay where they are: Access denied.",
    ),
  ).resolves.toBeDefined();
});

test.each([
  { state: "a disabled", pluginId: "old-store", name: "Old store" },
  { state: "an enabled", pluginId: "s3", name: "S3" },
])(
  "switching untouched from $state Blob store Plugin in use asks for it, not local disk",
  async ({ pluginId, name }) => {
    const switches: string[] = [];
    openSettings({
      getImageSettings: () => ({ settings: { storePluginId: pluginId } }),
      switchImageStore: ({ pluginId: asked }) => {
        switches.push(asked);
        throw refused(Code.FailedPrecondition, "the image store is already in use", {
          reason: RefusalReason.IMAGE_STORE_IN_USE,
          field: "plugin_id",
          params: { pluginId: asked },
        });
      },
    });

    const store = await imageStore();
    await store.findByRole("option", { name: "S3" });
    fireEvent.click(store.getByRole("button", { name: "Switch image store" }));

    await expect(store.findByText(`${name} already keeps the images.`)).resolves.toBeDefined();
    expect(switches).toEqual([pluginId]);
  },
);

test("picking local disk or S3 while a disabled Blob store Plugin is in use switches to the store picked", async () => {
  const switches: string[] = [];
  openSettings({
    getImageSettings: () => ({ settings: { storePluginId: "old-store" } }),
    switchImageStore: ({ pluginId }) => {
      switches.push(pluginId);
      return {};
    },
  });

  const store = await imageStore();
  await store.findByRole("option", { name: "S3" });
  switchTo(store, "");
  await expect.poll(() => switches).toEqual([""]);
  switchTo(store, "s3");
  await expect.poll(() => switches).toEqual(["", "s3"]);
});
