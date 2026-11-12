import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  type SetSubtitlePreferenceRequest,
  SubtitleKind,
  type UsersAudioPreferenceSchema,
  type UsersSubtitlePreferenceSchema,
} from "@/gen/nama/v1/playback_pb";

import { fakeServer, fill, newUser, openApp } from "./test-app";
import { expectUnderField } from "./test-refusal";

afterEach(cleanup);

/** A fake Server where the signed-in User's own Subtitle Preference is `subtitle`. */
function server(
  subtitle: MessageInitShape<typeof UsersSubtitlePreferenceSchema> = {},
  set: SetSubtitlePreferenceRequest[] = [],
) {
  return fakeServer({
    signedIn: newUser,
    playbackService: {
      getTrackPreferences: () => ({ subtitle }),
      setSubtitlePreference: (request) => {
        set.push(request);
        return {};
      },
    },
  });
}

test("a User whose subtitles are off picks a language by name, and which subtitles in it", async () => {
  const set: SetSubtitlePreferenceRequest[] = [];
  openApp("/settings", server({}, set));

  const subtitles = await screen.findByLabelText<HTMLInputElement>("Subtitles");
  expect(subtitles.value).toBe("");
  expect(screen.queryByRole("group", { name: "Which subtitles" })).toBeNull();
  // A datalist's options stay out of the accessibility tree.
  expect(subtitles.list?.querySelector("option[value=German]")).not.toBeNull();
  fill("Subtitles", "german");
  fireEvent.click(screen.getByLabelText("Forced only, for signs and foreign dialogue"));
  fireEvent.click(screen.getByRole("button", { name: "Save subtitles" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(set).toMatchObject([{ subtitle: { language: "de", kind: SubtitleKind.FORCED } }]);
});

test("a User's own subtitles show as they set them, and picking Off turns them off", async () => {
  const set: SetSubtitlePreferenceRequest[] = [];
  openApp("/settings", server({ language: "en", kind: SubtitleKind.SDH }, set));

  const subtitles = await screen.findByLabelText<HTMLInputElement>("Subtitles");
  expect(subtitles.value).toBe("English");
  expect(screen.getByLabelText<HTMLInputElement>("Full, SDH (also describes sounds)").checked).toBe(
    true,
  );
  fill("Subtitles", "Off");
  expect(screen.queryByRole("group", { name: "Which subtitles" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Save subtitles" }));

  expect(await screen.findByText("Saved.")).toBeDefined();
  expect(set).toHaveLength(1);
  expect(set[0]?.subtitle?.language).toBeUndefined();
});

test("a language not on the list is refused under the field, and nothing is saved", async () => {
  const set: SetSubtitlePreferenceRequest[] = [];
  openApp("/settings", server({}, set));

  await screen.findByLabelText("Subtitles");
  fill("Subtitles", "Klingon");
  fireEvent.click(screen.getByRole("button", { name: "Save subtitles" }));

  await expectUnderField("Subtitles", "“Klingon” isn’t a language on the list. Pick one from it.");
  expect(set).toEqual([]);
});

/** A fake Server that keeps the signed-in User's own Audio Preference, starting at `audio`. */
function keepingAudio(audio: MessageInitShape<typeof UsersAudioPreferenceSchema> = {}) {
  let kept = audio;
  return fakeServer({
    signedIn: newUser,
    playbackService: {
      getTrackPreferences: () => ({ audio: kept }),
      setAudioPreference: (request) => {
        kept = request.audio ?? {};
        return {};
      },
    },
  });
}

test("a User's audio starts at the file's default until they pick a language, which stays after a reload", async () => {
  const transport = keepingAudio();
  openApp("/settings", transport);

  const audio = await screen.findByLabelText<HTMLInputElement>("Audio");
  expect(audio.value).toBe("");
  expect(audio.placeholder).toBe("File’s default");
  fill("Audio", "Japanese");
  fireEvent.click(screen.getByRole("button", { name: "Save audio" }));
  expect(await screen.findByText("Saved.")).toBeDefined();
  cleanup();
  openApp("/settings", transport);

  expect((await screen.findByLabelText<HTMLInputElement>("Audio")).value).toBe("Japanese");
});

test("picking File’s default clears a User's audio language", async () => {
  const transport = keepingAudio({ language: "ja" });
  openApp("/settings", transport);

  await screen.findByLabelText("Audio");
  fill("Audio", "File’s default");
  fireEvent.click(screen.getByRole("button", { name: "Save audio" }));
  expect(await screen.findByText("Saved.")).toBeDefined();
  cleanup();
  openApp("/settings", transport);

  expect((await screen.findByLabelText<HTMLInputElement>("Audio")).value).toBe("");
});

test("an audio language not on the list is refused under the field", async () => {
  openApp("/settings", keepingAudio());

  await screen.findByLabelText("Audio");
  fill("Audio", "Klingon");
  fireEvent.click(screen.getByRole("button", { name: "Save audio" }));

  await expectUnderField("Audio", "“Klingon” isn’t a language on the list. Pick one from it.");
});
