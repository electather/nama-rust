const languageNames = new Intl.DisplayNames(["en"], { type: "language" });

/** The language a file tags as `code`, such as "English" for `eng`, if it tags one. */
export function languageName(code: string | undefined) {
  // ISO 639-2's `und`, undetermined, would be named "root".
  return code === undefined || code === "und" ? undefined : nameOf(code);
}

function nameOf(code: string) {
  try {
    return languageNames.of(code);
  } catch {
    // A tag that isn't a language code at all is shown as it is.
    return code;
  }
}
