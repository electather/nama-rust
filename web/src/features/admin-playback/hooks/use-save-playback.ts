import { type ChangeEvent, type FocusEvent, type KeyboardEvent, useRef } from "react";

import { useUpdatePlaybackSettings } from "@/features/admin-playback/data/use-update-playback-settings";
import { typedSettings } from "@/features/admin-playback/lib/playback-settings-form";

/**
 * Saves the Playback settings form as it's changed: a dropdown as it's picked, a field once it's
 * left or Enter is pressed in it. Each save sends the whole form, as saving replaces every Playback
 * setting. `error` is why the Server refused the last one.
 */
export function useSavePlayback() {
  const update = useUpdatePlaybackSettings();
  // One save at a time, in the order the changes were made, so a slow one never lands over a newer.
  const last = useRef(Promise.resolve());

  /** Saves what `form` holds once the saves before it are `done`. It never rejects. */
  async function send(done: Promise<void>, form: HTMLFormElement) {
    await done;
    try {
      await update.mutateAsync({ settings: typedSettings(new FormData(form)) });
    } catch {
      // `update.error` keeps the refusal, to show under its field.
    }
  }
  const save = (form: HTMLFormElement | null) => {
    if (form !== null) {
      last.current = send(last.current, form);
    }
  };
  // A field's `defaultValue` is what's saved, so leaving one unchanged saves nothing.
  const saveField = (field: HTMLInputElement) => {
    if (field.value !== field.defaultValue) {
      save(field.form);
    }
  };

  return {
    error: update.error,
    onPick: (event: ChangeEvent<HTMLSelectElement>) => {
      save(event.currentTarget.form);
    },
    /** What a text or number field saves on. */
    field: {
      onBlur: (event: FocusEvent<HTMLInputElement>) => {
        saveField(event.currentTarget);
      },
      onKeyDown: (event: KeyboardEvent<HTMLInputElement>) => {
        if (event.key === "Enter") {
          saveField(event.currentTarget);
        }
      },
    },
  };
}
