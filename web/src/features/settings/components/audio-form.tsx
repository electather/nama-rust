import { LanguageField } from "@/features/settings/components/language-field";
import { useSaveAudioPreference } from "@/features/settings/data/use-save-audio-preference";
import { useTypedLanguage } from "@/features/settings/hooks/use-typed-language";
import { FILES_DEFAULT, wantedAudio } from "@/features/settings/lib/track-language";
import type { UsersAudioPreference } from "@/gen/nama/v1/playback_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { onSubmitted } from "@/shared/lib/forms";

/** The form a User sets their own Audio Preference with, starting at the one `saved`. */
export function AudioForm({ saved }: { saved: UsersAudioPreference | undefined }) {
  const save = useSaveAudioPreference();
  const field = useTypedLanguage(saved?.language, save, "save your audio");
  const { language, setUnlisted } = field;
  return (
    <form
      className="mt-8 flex flex-col gap-5"
      onSubmit={onSubmitted(() => {
        const audio = wantedAudio(language);
        setUnlisted(audio === undefined);
        if (audio !== undefined) {
          save.mutate({ audio });
        }
      })}
    >
      <LanguageField
        label="Audio"
        none={FILES_DEFAULT}
        hint="Type to find a language, or pick File’s default for the audio each file starts with."
        value={language}
        error={field.error}
        onChange={field.handleChange}
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
