import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { SettingKind } from "@/gen/nama/v1/plugin_pb";
import { admin, fakeServer, newUser, openApp } from "@/test-app";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

/** Types `words` into Settings' search, once it's open. */
async function search(words: string) {
  const field = await screen.findByRole<HTMLInputElement>("combobox", { name: "Search settings" });
  fireEvent.change(field, { target: { value: words } });
  return field;
}

/** Each result listed, as the Section's name then the setting's. */
function results() {
  return screen.queryAllByRole("option").map((option) => option.textContent);
}

/** The setting's row that has focus, by its label. */
function focusedRow() {
  return document.activeElement?.querySelector("label, h4, p")?.textContent;
}

/** ntfy's one Personal setting, its topic. */
const ntfy = {
  pluginId: "ntfy",
  name: "ntfy",
  settings: [{ key: "topic", label: "Topic", description: "", kind: SettingKind.TEXT }],
};

test("a word from a setting's hint finds it under its Section, and says how many", async () => {
  openSettings("devices", fakeServer({ signedIn: newUser }));

  await search("door SLAMS");

  expect(results()).toStrictEqual(["PlaybackInclude sounds (SDH)"]);
  expect(screen.getByText("1 result")).toBeDefined();
});

test("every word must start a word, ignoring case and accents, and the letters are marked", async () => {
  openSettings("devices", fakeServer({ signedIn: newUser }));

  await search("subtítle lang");

  expect(results()).toStrictEqual([
    "PlaybackSubtitle language",
    "PlaybackWhich subtitles",
    "PlaybackOnly for audio in another language",
  ]);
  const option = screen.getByRole("option", { name: "Subtitle language, Playback" });
  const marked = [...option.querySelectorAll("mark")].map((mark) => mark.textContent);
  expect(marked).toStrictEqual(["Subtitle", "lang"]);
});

test("a Section's name finds the Section, and a word in nothing says so", async () => {
  openSettings("playback", fakeServer({ signedIn: newUser }));

  await search("devic");
  expect(results()).toStrictEqual(["Devices", "DevicesThis Device", "DevicesOther Devices"]);
  await search("wallpaper");

  expect(results()).toStrictEqual([]);
  expect(screen.getByText("No settings match “wallpaper”.")).toBeDefined();
});

test("only settings in Sections the User may open are found", async () => {
  openSettings("playback", fakeServer({ signedIn: newUser }));

  await search("backup folder");

  expect(results()).toStrictEqual([]);
});

test("choosing a result opens its row, focused and marked, and clears the field", async () => {
  const router = openSettings("devices", fakeServer({ signedIn: admin }));

  const field = await search("outline");
  fireEvent.click(screen.getByRole("option", { name: /Subtitle background/u }));

  await waitFor(() => {
    expect(focusedRow()).toBe("Subtitle background");
  });
  expect(router.state.location.search).toMatchObject({
    settings: "playback",
    setting: "background",
  });
  expect(document.querySelector("[data-found]")).toBe(document.activeElement);
  expect(field.value).toBe("");
});

test("a row shown only sometimes is found, and lands on the row that shows it", async () => {
  const router = openSettings("devices", fakeServer({ signedIn: newUser }));

  await search("sdh");
  fireEvent.click(screen.getByRole("option", { name: /SDH/u }));

  await waitFor(() => {
    expect(focusedRow()).toBe("Subtitle language");
  });
  expect(router.state.location.search).toMatchObject({ setting: "sdh" });
});

test("arrow keys move through the results, Enter opens one and Escape clears", async () => {
  const router = openSettings("devices", fakeServer({ signedIn: newUser }));

  const field = await search("subtitle");
  fireEvent.keyDown(field, { key: "ArrowDown" });
  fireEvent.keyDown(field, { key: "ArrowDown" });
  fireEvent.keyDown(field, { key: "ArrowUp" });
  expect(field.getAttribute("aria-activedescendant")).toBe(screen.getAllByRole("option")[1]?.id);
  fireEvent.keyDown(field, { key: "Escape" });
  expect(field.value).toBe("");
  await search("subtitle");
  fireEvent.keyDown(field, { key: "Enter" });

  await waitFor(() => {
    expect(focusedRow()).toBe("Subtitle language");
  });
  expect(router.state.location.search).toMatchObject({ settings: "playback" });
});

test("settings the Server defines are asked for on the first keystroke, and join the results", async () => {
  let asked = 0;
  const router = openSettings(
    "devices",
    fakeServer(
      { signedIn: newUser },
      {},
      {
        listPersonalSettings: () => {
          asked += 1;
          return { plugins: [ntfy] };
        },
      },
    ),
  );

  const field = await search("");
  expect(asked).toBe(0);
  fireEvent.change(field, { target: { value: "topic" } });
  fireEvent.click(await screen.findByRole("option", { name: "Topic, Connections" }));

  await waitFor(() => {
    expect(focusedRow()).toBe("Topic");
  });
  expect(router.state.location.search).toMatchObject({
    settings: "connections",
    setting: "personal.ntfy.topic",
  });
});

test("an address naming no setting opens the Section at its top", async () => {
  openApp("/code?settings=playback&setting=nonsense", fakeServer({ signedIn: newUser }));

  await expect(screen.findByRole("heading", { name: "Playback" })).resolves.toBeDefined();
  expect(document.activeElement).toBe(screen.getByRole("combobox", { name: "Search settings" }));
});

test("a Home row is found by its heading, and lands on its row in the Home layout", async () => {
  const topPicks = { pluginId: "top-picks", key: "picks", heading: "Top picks" };
  const router = openSettings(
    "devices",
    fakeServer(
      { signedIn: newUser },
      {},
      {
        getHomeLayout: () => ({ layout: { rows: [{ row: { case: "plugin", value: topPicks } }] } }),
      },
    ),
  );

  await search("top");
  fireEvent.click(await screen.findByRole("option", { name: "Top picks, Home" }));

  await waitFor(() => {
    expect(focusedRow()).toBe("Top picks");
  });
  expect(router.state.location.search).toMatchObject({
    settings: "home",
    setting: "row.plugin/top-picks/picks",
  });
});

test("choosing the setting already open lands on it again", async () => {
  openSettings("devices", fakeServer({ signedIn: newUser }));

  const field = await search("outline");
  fireEvent.click(screen.getByRole("option", { name: /Subtitle background/u }));
  await waitFor(() => {
    expect(focusedRow()).toBe("Subtitle background");
  });
  field.focus();
  await search("outline");
  fireEvent.click(screen.getByRole("option", { name: /Subtitle background/u }));

  await waitFor(() => {
    expect(focusedRow()).toBe("Subtitle background");
  });
});

test("the Image store's picker is found by its hint, and lands on its row", async () => {
  const router = openSettings("devices", fakeServer({ signedIn: admin }));

  await search("s3");
  expect(results()).toStrictEqual(["ServerKeep images on"]);
  fireEvent.click(screen.getByRole("option", { name: "Keep images on, Server" }));

  await waitFor(() => {
    expect(focusedRow()).toBe("Keep images on");
  });
  expect(router.state.location.search).toMatchObject({ settings: "server", setting: "store" });
});
