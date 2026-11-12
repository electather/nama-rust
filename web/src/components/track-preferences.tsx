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
 * The signed-in User's own Track Preferences: the subtitles everything they play starts with,
 * unless they picked others in that Show or Movie (ADR 0035).
 */
export function TrackPreferences() {
  const { data } = useSuspenseQuery(PlaybackService.method.getTrackPreferences, {});
  return (
    <section aria-labelledby="playback" className="mt-12 max-w-sm">
      <h2 id="playback" className="font-medium">
        Playback
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        What everything you play starts with, on every Device. A subtitle you pick while watching a
        Show or Movie is kept for that one instead.
      </p>
      <SubtitlesForm saved={data.subtitle} />
    </section>
  );
}

/** The form a User sets their own Subtitle Preference with, starting at the one `saved`. */
function SubtitlesForm({ saved }: { saved: UsersSubtitlePreference | undefined }) {
  const queryClient = useQueryClient();
  const save = useMutation(PlaybackService.method.setSubtitlePreference, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: preferencesKey }),
  });
  const [language, setLanguage] = useState(() => languageName(saved?.language ?? ""));
  const [unlisted, setUnlisted] = useState(false);
  const errors = subtitleErrors(save.error, language);
  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        const subtitle = wanted(language, formText(typed, "kind"));
        setUnlisted(subtitle === undefined);
        if (subtitle !== undefined) save.mutate({ subtitle });
      })}
    >
      <LanguageField
        value={language}
        error={unlisted ? unknownLanguage(language) : errors.subtitleLanguage}
        onChange={(typed) => {
          setLanguage(typed);
          setUnlisted(false);
          save.reset();
        }}
      />
      {!isOff(language) && (
        <Choices
          legend="Which subtitles"
          name="kind"
          choices={kinds.map(([kind, label]) => [String(kind), label])}
          chosen={savedKind(saved)}
        />
      )}
      <FormEnd
        action="Save subtitles"
        variant="ghost"
        pending={save.isPending}
        done={save.isSuccess ? "Saved." : undefined}
        refusal={errors.form}
      />
    </form>
  );
}

/** The field a language is typed in, suggesting Off and each language on the list by name. */
function LanguageField({
  value,
  error,
  onChange,
}: {
  value: string;
  error: string | undefined;
  onChange: (typed: string) => void;
}) {
  const listId = useId();
  return (
    <>
      <FormField
        label="Subtitles"
        name="language"
        required={false}
        list={listId}
        autoComplete="off"
        placeholder="Off"
        hint="Type to find a language, or pick Off for no subtitles."
        error={error}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      <datalist id={listId}>
        <option value="Off">Off</option>
        {languages.map(({ code, name }) => (
          <option key={code} value={name}>
            {name}
          </option>
        ))}
      </datalist>
    </>
  );
}

/** Whether `typed` turns subtitles off: nothing typed, or Off picked. */
function isOff(typed: string) {
  return ["", "off"].includes(typed.trim().toLowerCase());
}

/**
 * The Subtitle Preference the language `typed` and the kind chosen as `kind` ask for, `undefined`
 * for a language not on the list.
 */
function wanted(typed: string, kind: string) {
  if (isOff(typed)) return {};
  const language = languageCode(typed);
  return language === undefined ? undefined : { language, kind: kindOf(kind) };
}

/** The kind of subtitle `saved` wants, as its radio button is valued, if it wants any. */
function savedKind(saved: UsersSubtitlePreference | undefined) {
  return saved?.language === undefined ? undefined : String(saved.kind);
}

/** Why saving the language `typed` was refused with `error`, if it was. */
function subtitleErrors(error: unknown, typed: string) {
  if (error === null || error === undefined) return {};
  return refusal(
    error,
    "save your subtitles",
    {},
    {
      [RefusalReason.UNKNOWN_LANGUAGE]: { subtitleLanguage: unknownLanguage(typed) },
    },
  );
}

/** Says the language `typed` isn't one to pick. */
function unknownLanguage(typed: string) {
  return `“${typed.trim()}” isn’t a language on the list. Pick one from it.`;
}
