import type { FoundSubtitle } from "@/gen/nama/v1/playback_pb";

/**
 * The languages to search for subtitles in: the User's Subtitle Preference's, if they set one,
 * then the browser's. Each is a bare language, as `en` for `en-US`, which every source knows.
 */
export function searchLanguages(preferred: string | undefined, browser: readonly string[]) {
  const tags = preferred === undefined ? browser : [preferred, ...browser];
  return [...new Set(tags.map((tag) => bareLanguage(tag)))];
}

/** `found`, those in the `preferred` language first, each group in the order the Server gave. */
export function preferredFirst(found: FoundSubtitle[], preferred: string | undefined) {
  const wanted = preferred === undefined ? undefined : bareLanguage(preferred);
  const isPreferred = ({ language }: FoundSubtitle) => bareLanguage(language) === wanted;
  return [
    ...found.filter((each) => isPreferred(each)),
    ...found.filter((each) => !isPreferred(each)),
  ];
}

/** The language `tag` names, as its shortest ISO 639 code: `en` for `eng` or `en-US`. */
function bareLanguage(tag: string) {
  try {
    return new Intl.Locale(tag).language;
  } catch {
    // A tag that isn't a language tag is compared as it is.
    return tag;
  }
}
