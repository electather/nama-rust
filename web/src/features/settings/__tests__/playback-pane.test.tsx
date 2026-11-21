import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  type SetAudioPreferenceRequest,
  type SetSubtitlePreferenceRequest,
  SubtitleBackground,
  SubtitleKind,
  SubtitleSize,
  type UsersAudioPreferenceSchema,
  type UsersSubtitlePreferenceSchema,
} from "@/gen/nama/v1/playback_pb";
import { fakeServer, newUser } from "@/test-app";
import { openSettings, pick } from "@/test-settings";

afterEach(cleanup);

const ONLY_OTHER = "Only for audio in another language";
const SDH = "Include sounds (SDH)";

/** A fake Server that keeps the signed-in User's own Subtitle Preference, starting at `subtitle`. */
function server(
  subtitle: MessageInitShape<typeof UsersSubtitlePreferenceSchema> = {},
  set: SetSubtitlePreferenceRequest[] = [],
) {
  let kept = subtitle;
  return fakeServer({
    signedIn: newUser,
    playbackService: {
      getTrackPreferences: () => ({ subtitle: kept }),
      setSubtitlePreference: (request) => {
        set.push(request);
        kept = request.subtitle ?? {};
        return {};
      },
    },
  });
}

function isOn(name: string) {
  return screen.getByRole("switch", { name }).getAttribute("aria-checked");
}

test("a User whose subtitles are off picks a language, and which subtitles in it", async () => {
  const set: SetSubtitlePreferenceRequest[] = [];
  openSettings("playback", server({}, set));

  const subtitles = await screen.findByRole("combobox", { name: "Subtitle language" });
  expect(subtitles.textContent).toBe("Off");
  expect(screen.queryByRole("radiogroup", { name: "Which subtitles" })).toBeNull();
  pick("Subtitle language", "German");
  fireEvent.click(screen.getByRole("radio", { name: "Forced only" }));

  // Each change is saved as it's made, in the order made.
  await expect
    .poll(() => set)
    .toMatchObject([
      { subtitle: { language: "de", kind: SubtitleKind.FULL } },
      { subtitle: { language: "de", kind: SubtitleKind.FORCED } },
    ]);
  // A change that's saved says nothing.
  expect(screen.queryByText("Not saved")).toBeNull();
  expect(screen.queryByText("Saved")).toBeNull();
});

test("a User's own subtitles show as they set them, and picking Off turns them off", async () => {
  const set: SetSubtitlePreferenceRequest[] = [];
  openSettings("playback", server({ language: "en", kind: SubtitleKind.SDH }, set));

  const subtitles = await screen.findByRole("combobox", { name: "Subtitle language" });
  expect(subtitles.textContent).toBe("English");
  expect(isOn(SDH)).toBe("true");
  pick("Subtitle language", "Off");
  expect(screen.queryByRole("radiogroup", { name: "Which subtitles" })).toBeNull();
  expect(screen.queryByRole("switch", { name: ONLY_OTHER })).toBeNull();

  await expect.poll(() => set).toHaveLength(1);
  expect(set[0]?.subtitle?.language).toBeUndefined();
});

test("full subtitles start only over audio in another language until that's switched off", async () => {
  const set: SetSubtitlePreferenceRequest[] = [];
  openSettings("playback", server({}, set));

  await screen.findByLabelText("Subtitle language");
  pick("Subtitle language", "German");
  expect(isOn(ONLY_OTHER)).toBe("true");
  fireEvent.click(screen.getByRole("switch", { name: ONLY_OTHER }));

  await expect
    .poll(() => set)
    .toMatchObject([
      { subtitle: { language: "de", kind: SubtitleKind.FULL, foreignAudioOnly: true } },
      { subtitle: { language: "de", kind: SubtitleKind.FULL, foreignAudioOnly: false } },
    ]);
});

test("choosing Forced only switches off what only Full subtitles take, saying why", async () => {
  const set: SetSubtitlePreferenceRequest[] = [];
  openSettings(
    "playback",
    server({ language: "en", kind: SubtitleKind.SDH, foreignAudioOnly: true }, set),
  );

  await screen.findByRole("switch", { name: ONLY_OTHER });
  expect([isOn(ONLY_OTHER), isOn(SDH)]).toStrictEqual(["true", "true"]);
  fireEvent.click(screen.getByRole("radio", { name: "Forced only" }));
  expect([isOn(ONLY_OTHER), isOn(SDH)]).toStrictEqual(["false", "false"]);
  expect(screen.getAllByText("Only applies to Full subtitles.")).toHaveLength(2);

  await expect
    .poll(() => set)
    .toMatchObject([{ subtitle: { kind: SubtitleKind.FORCED, foreignAudioOnly: false } }]);
});

/** A fake Server that keeps the signed-in User's own Audio Preference, starting at `audio`. */
function keepingAudio(set: SetAudioPreferenceRequest[]) {
  let kept: MessageInitShape<typeof UsersAudioPreferenceSchema> = {};
  return fakeServer({
    signedIn: newUser,
    playbackService: {
      getTrackPreferences: () => ({ audio: kept }),
      setAudioPreference: (request) => {
        set.push(request);
        kept = request.audio ?? {};
        return {};
      },
    },
  });
}

test("a User's audio starts at the file's default until they pick a language, which stays after a reload", async () => {
  const set: SetAudioPreferenceRequest[] = [];
  const transport = keepingAudio(set);
  openSettings("playback", transport);

  const audio = await screen.findByRole("combobox", { name: "Audio language" });
  expect(audio.textContent).toBe("File’s default");
  pick("Audio language", "Japanese");
  await expect.poll(() => set).toHaveLength(1);
  cleanup();
  openSettings("playback", transport);

  const reloaded = await screen.findByRole("combobox", { name: "Audio language" });
  expect(reloaded.textContent).toBe("Japanese");
});

test("the Subtitle Appearance and autoplay are saved as they change, each with its own call", async () => {
  const calls: string[] = [];
  const transport = fakeServer({
    signedIn: newUser,
    playbackService: {
      getSubtitleAppearance: () => ({
        size: SubtitleSize.LARGE,
        background: SubtitleBackground.OUTLINE,
      }),
      getAutoplay: () => ({ on: false }),
      setSubtitleSize: ({ size }) => {
        calls.push(`size ${size}`);
        return {};
      },
      setSubtitleBackground: ({ background }) => {
        calls.push(`background ${background}`);
        return {};
      },
      setAutoplay: ({ on }) => {
        calls.push(`autoplay ${on}`);
        return {};
      },
    },
  });
  openSettings("playback", transport);

  const large = await screen.findByRole<HTMLInputElement>("radio", { name: "Large" });
  expect(large.checked).toBe(true);
  expect(isOn("Play the next Episode automatically")).toBe("false");
  fireEvent.click(screen.getByRole("radio", { name: "Largest" }));
  fireEvent.click(screen.getByRole("switch", { name: "Play the next Episode automatically" }));

  await expect.poll(() => calls).toStrictEqual([`size ${SubtitleSize.LARGEST}`, "autoplay true"]);
});

test("a change the Server refuses goes back to what's saved, with a toast saying why until dismissed", async () => {
  const transport = fakeServer({
    signedIn: newUser,
    playbackService: {
      setSubtitleSize: () => {
        throw new ConnectError("the database is locked", Code.Internal);
      },
    },
  });
  openSettings("playback", transport);

  const small = await screen.findByRole<HTMLInputElement>("radio", { name: "Small" });
  fireEvent.click(small);
  // It shows at once, before the Server answers.
  expect(small.checked).toBe(true);

  await expect(
    screen.findByText(
      "Couldn’t save Subtitle size. Check that the Server is running, then try again.",
    ),
  ).resolves.toBeDefined();
  expect(screen.getByText("Not saved")).toBeDefined();
  expect(small.checked).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
  expect(screen.queryByText("Not saved")).toBeNull();
});

test("a change is saved even when its pane is left at once, with nothing asked", async () => {
  const set: SetSubtitlePreferenceRequest[] = [];
  openSettings("playback", server({}, set));

  await screen.findByLabelText("Subtitle language");
  pick("Subtitle language", "German");
  fireEvent.click(screen.getByRole("link", { name: "Devices" }));

  await expect(screen.findByRole("heading", { name: "Devices" })).resolves.toBeDefined();
  await expect.poll(() => set).toMatchObject([{ subtitle: { language: "de" } }]);
});
