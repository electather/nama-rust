import type { Transport } from "@connectrpc/connect";
import {
  createConnectQueryKey,
  createQueryOptions,
  useMutation,
  useSuspenseQuery,
} from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";

import { Choices, FormEnd, FormField } from "@/components/form-field";
import {
  PlaybackService,
  SubtitleKind,
  type UsersAudioPreference,
  type UsersSubtitlePreference,
} from "@/gen/nama/v1/playback_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { formText, onSubmitted, refusal } from "@/lib/forms";
import { languageCode, languageName, languages } from "@/lib/languages";

const preferencesKey = createConnectQueryKey({
  schema: PlaybackService.method.getTrackPreferences,
  cardinality: "finite",
});

/** The signed-in User's own Track Preferences, which Settings loads before it shows. */
export function trackPreferencesQuery(transport: Transport) {
  return createQueryOptions(PlaybackService.method.getTrackPreferences, {}, { transport });
}

/** What the language field says for subtitles off. */
const OFF = "Off";
/** What the language field says for the audio the file plays first. */
const FILES_DEFAULT = "File’s default";

/** Each kind of subtitle a User's own Subtitle Preference can want, with its label. */
const kinds: [kind: SubtitleKind, label: string][] = [
  [SubtitleKind.FULL, "Full"],
  [SubtitleKind.SDH, "Full, SDH (also describes sounds)"],
  [SubtitleKind.FORCED, "Forced only, for signs and foreign dialogue"],
];

/** The kind of subtitle the radio button valued `value` stands for. */
function kindOf(value: string) {
  return kinds.find(([kind]) => String(kind) === value)?.[0] ?? SubtitleKind.FULL;
}

/**
 * The signed-in User's own Track Preferences: the subtitles and audio everything they play starts
 * with, unless they picked others in that Show or Movie (ADR 0035).
 */
export function TrackPreferences() {
  const { data } = useSuspenseQuery(PlaybackService.method.getTrackPreferences, {});
  return (
    <section aria-labelledby="playback" className="mt-12 max-w-sm">
      <h2 id="playback" className="font-medium">
        Playback
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        What everything you play starts with, on every Device. Subtitles or audio you pick while
        watching a Show or Movie are kept for that one instead.
      </p>
      <SubtitlesForm saved={data.subtitle} />
      <AudioForm saved={data.audio} />
    </section>
  );
}

/** The form a User sets their own Subtitle Preference with, starting at the one `saved`. */
function SubtitlesForm({ saved }: { saved: UsersSubtitlePreference | undefined }) {
  const queryClient = useQueryClient();
  const save = useMutation(PlaybackService.method.setSubtitlePreference, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: preferencesKey }),
  });
  const field = useTypedLanguage(saved?.language, save, "save your subtitles");
  const { language, setUnlisted } = field;
  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        const subtitle = wanted(language, typed);
        setUnlisted(subtitle === undefined);
        if (subtitle !== undefined) save.mutate({ subtitle });
      })}
    >
      <LanguageField
        label="Subtitles"
        none={OFF}
        hint="Type to find a language, or pick Off for no subtitles."
        value={language}
        error={field.error}
        onChange={field.onChange}
      />
      {!isNone(language, OFF) && <WhichSubtitles saved={saved} />}
      <FormEnd
        action="Save subtitles"
        variant="ghost"
        pending={save.isPending}
        done={save.isSuccess ? "Saved." : undefined}
        refusal={field.formError}
      />
    </form>
  );
}

/**
 * Which subtitles in the language picked a User wants, starting at the ones `saved` wants, and
 * whether only over audio in another language, which forced-only subtitles are for anyway.
 */
function WhichSubtitles({ saved }: { saved: UsersSubtitlePreference | undefined }) {
  const [kind, setKind] = useState(savedKind(saved) ?? String(SubtitleKind.FULL));
  return (
    <>
      <Choices
        legend="Which subtitles"
        name="kind"
        choices={kinds.map(([each, label]) => [String(each), label])}
        chosen={kind}
        onChoose={setKind}
      />
      {kindOf(kind) !== SubtitleKind.FORCED && (
        <label className="flex items-center gap-3 text-sm">
          <input
            type="checkbox"
            name="foreignAudioOnly"
            defaultChecked={saved?.foreignAudioOnly}
            className="size-4 accent-foreground"
          />
          Only when the audio is in another language
        </label>
      )}
    </>
  );
}

/** The form a User sets their own Audio Preference with, starting at the one `saved`. */
function AudioForm({ saved }: { saved: UsersAudioPreference | undefined }) {
  const queryClient = useQueryClient();
  const save = useMutation(PlaybackService.method.setAudioPreference, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: preferencesKey }),
  });
  const field = useTypedLanguage(saved?.language, save, "save your audio");
  const { language, setUnlisted } = field;
  return (
    <form
      className="mt-8 flex flex-col gap-5"
      onSubmit={onSubmitted(() => {
        const audio = wantedAudio(language);
        setUnlisted(audio === undefined);
        if (audio !== undefined) save.mutate({ audio });
      })}
    >
      <LanguageField
        label="Audio"
        none={FILES_DEFAULT}
        hint="Type to find a language, or pick File’s default for the audio each file starts with."
        value={language}
        error={field.error}
        onChange={field.onChange}
      />
      <FormEnd
        action="Save audio"
        variant="ghost"
        pending={save.isPending}
        done={save.isSuccess ? "Saved." : undefined}
        refusal={field.formError}
      />
    </form>
  );
}

/**
 * The language typed in a form that tries to `attempt` with `save`, starting at the one `saved`
 * names: what typing in it does, and why the field or the form was refused, if it was.
 * `setUnlisted` marks it as not on the list, which the form finds before it saves.
 */
function useTypedLanguage(
  saved: string | undefined,
  save: { reset: () => void; error: unknown },
  attempt: string,
) {
  const [language, setLanguage] = useState(() => languageName(saved ?? ""));
  const [unlisted, setUnlisted] = useState(false);
  const errors = languageErrors(save.error, language, attempt);
  const onChange = (typed: string) => {
    setLanguage(typed);
    setUnlisted(false);
    save.reset();
  };
  const error = unlisted ? unknownLanguage(language) : errors.language;
  return { language, setUnlisted, onChange, error, formError: errors.form };
}

/**
 * The field labelled `label` a language is typed in, suggesting `none`, for no language, and each
 * language on the list by name.
 */
function LanguageField({
  label,
  none,
  hint,
  value,
  error,
  onChange,
}: {
  label: string;
  none: string;
  hint: string;
  value: string;
  error: string | undefined;
  onChange: (typed: string) => void;
}) {
  const listId = useId();
  return (
    <>
      <FormField
        label={label}
        name="language"
        required={false}
        list={listId}
        autoComplete="off"
        placeholder={none}
        hint={hint}
        error={error}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      <datalist id={listId}>
        <option value={none}>{none}</option>
        {languages.map(({ code, name }) => (
          <option key={code} value={name}>
            {name}
          </option>
        ))}
      </datalist>
    </>
  );
}

/** Whether `typed` names no language: nothing typed, or `none` picked or typed in any case. */
function isNone(typed: string, none: string) {
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
function wanted(typed: string, chosen: FormData) {
  if (isNone(typed, OFF)) return {};
  const language = languageCode(typed);
  const kind = kindOf(formText(chosen, "kind"));
  const foreignAudioOnly = chosen.has("foreignAudioOnly");
  return language === undefined ? undefined : { language, kind, foreignAudioOnly };
}

/** The Audio Preference the language `typed` asks for, `undefined` for a language not on the list. */
function wantedAudio(typed: string) {
  if (isNone(typed, FILES_DEFAULT)) return {};
  const language = languageCode(typed);
  return language === undefined ? undefined : { language };
}

/** The kind of subtitle `saved` wants, as its radio button is valued, if it wants any. */
function savedKind(saved: UsersSubtitlePreference | undefined) {
  return saved?.language === undefined ? undefined : String(saved.kind);
}

/** Why the `attempt` to save the language `typed` was refused with `error`, if it was. */
function languageErrors(error: unknown, typed: string, attempt: string) {
  if (error === null || error === undefined) return {};
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
function unknownLanguage(typed: string) {
  return `“${typed.trim()}” isn’t a language on the list. Pick one from it.`;
}
