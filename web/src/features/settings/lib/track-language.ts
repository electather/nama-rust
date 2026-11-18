import { languageCode } from "@/features/settings/lib/languages";
import { SubtitleKind, type UsersSubtitlePreference } from "@/gen/nama/v1/playback_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { formText, refusal } from "@/shared/lib/forms";

/** What the language field says for subtitles off. */
export const OFF = "Off";
/** What the language field says for the audio the file plays first. */
export const FILES_DEFAULT = "File’s default";

/** Each kind of subtitle a User's own Subtitle Preference can want, with its label. */
export const kinds: [kind: SubtitleKind, label: string][] = [
  [SubtitleKind.FULL, "Full"],
  [SubtitleKind.SDH, "Full, SDH (also describes sounds)"],
  [SubtitleKind.FORCED, "Forced only, for signs and foreign dialogue"],
];

/** The kind of subtitle the radio button valued `value` stands for. */
export function kindOf(value: string) {
  return kinds.find(([kind]) => String(kind) === value)?.[0] ?? SubtitleKind.FULL;
}

/** Whether `typed` names no language: nothing typed, or `none` picked or typed in any case. */
export function isNone(typed: string, none: string) {
  return ["", comparable(none)].includes(comparable(typed));
}

/** `text` as typed words are compared: trimmed, in lower case, with curly apostrophes. */
function comparable(text: string) {
  // A keyboard types a straight apostrophe where `FILES_DEFAULT` has a curly one.
  return text.trim().toLowerCase().replaceAll("'", "’");
}

/**
 * The Subtitle Preference the language `typed` and the rest of the form `chosen` ask for,
 * `undefined` for a language not on the list. The switch is off unless shown and checked.
 */
export function wanted(typed: string, chosen: FormData) {
  if (isNone(typed, OFF)) {
    return {};
  }
  const language = languageCode(typed);
  const kind = kindOf(formText(chosen, "kind"));
  const foreignAudioOnly = chosen.has("foreignAudioOnly");
  return language === undefined ? undefined : { language, kind, foreignAudioOnly };
}

/** The Audio Preference the language `typed` asks for, `undefined` for a language not on the list. */
export function wantedAudio(typed: string) {
  if (isNone(typed, FILES_DEFAULT)) {
    return {};
  }
  const language = languageCode(typed);
  return language === undefined ? undefined : { language };
}

/** The kind of subtitle `saved` wants, as its radio button is valued, if it wants any. */
export function savedKind(saved: UsersSubtitlePreference | undefined) {
  return saved?.language === undefined ? undefined : String(saved.kind);
}

/** Why the `attempt` to save the language `typed` was refused with `error`, if it was. */
export function languageErrors(error: unknown, typed: string, attempt: string) {
  if (error === null || error === undefined) {
    return {};
  }
  return refusal(
    error,
    attempt,
    {},
    {
      [RefusalReason.UNKNOWN_LANGUAGE]: { language: unknownLanguage(typed) },
    },
  );
}

/** Says the language `typed` isn't one to pick. */
export function unknownLanguage(typed: string) {
  return `“${typed.trim()}” isn’t a language on the list. Pick one from it.`;
}
