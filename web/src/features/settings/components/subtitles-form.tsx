import { LanguageField } from "@/features/settings/components/language-field";
import { WhichSubtitles } from "@/features/settings/components/which-subtitles";
import { useSaveSubtitlePreference } from "@/features/settings/data/use-save-subtitle-preference";
import { useTypedLanguage } from "@/features/settings/hooks/use-typed-language";
import { OFF, isNone, wanted } from "@/features/settings/lib/track-language";
import type { UsersSubtitlePreference } from "@/gen/nama/v1/playback_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { onSubmitted } from "@/shared/lib/forms";

/** The form a User sets their own Subtitle Preference with, starting at the one `saved`. */
export function SubtitlesForm({ saved }: { saved: UsersSubtitlePreference | undefined }) {
  const save = useSaveSubtitlePreference();
  const field = useTypedLanguage(saved?.language, save, "save your subtitles");
  const { language, setUnlisted } = field;
  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        const subtitle = wanted(language, typed);
        setUnlisted(subtitle === undefined);
        if (subtitle !== undefined) {
          save.mutate({ subtitle });
        }
      })}
    >
      <LanguageField
        label="Subtitles"
        none={OFF}
        hint="Type to find a language, or pick Off for no subtitles."
        value={language}
        error={field.error}
        onChange={field.handleChange}
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
