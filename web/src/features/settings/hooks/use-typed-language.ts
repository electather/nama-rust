import { useState } from "react";

import { languageName } from "@/features/settings/lib/languages";
import { languageErrors, unknownLanguage } from "@/features/settings/lib/track-language";

/**
 * The language typed in a form that tries to `attempt` with `save`, starting at the one `saved`
 * names: what typing in it does, and why the field or the form was refused, if it was.
 * `setUnlisted` marks it as not on the list, which the form finds before it saves.
 */
export function useTypedLanguage(
  saved: string | undefined,
  save: { reset: () => void; error: unknown },
  attempt: string,
) {
  const [language, setLanguage] = useState(() => languageName(saved ?? ""));
  const [unlisted, setUnlisted] = useState(false);
  const errors = languageErrors(save.error, language, attempt);
  const handleChange = (typed: string) => {
    setLanguage(typed);
    setUnlisted(false);
    save.reset();
  };
  const error = unlisted ? unknownLanguage(language) : errors.language;
  return { language, setUnlisted, handleChange, error, formError: errors.form };
}
