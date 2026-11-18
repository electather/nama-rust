import { create } from "@bufbuild/protobuf";

import { type Track, TrackSchema } from "@/gen/nama/v1/browse_pb";

const languageNames = new Intl.DisplayNames(["en"], { type: "language" });

/**
 * A subtitle or audio Track's name, its language or else its title, and what else tells it apart:
 * its title, Forced and SDH.
 */
export function trackLabel(track: Track | undefined) {
  const facts = track ?? create(TrackSchema);
  const language = languageName(facts.language);
  return {
    name: language ?? facts.title ?? "Unknown language",
    details: details(facts, language !== undefined),
  };
}

/** Its title, unless that's its name, then Forced and SDH. */
function details({ title, forced, sdh }: Track, named: boolean) {
  return [named && title, forced && "Forced", sdh && "SDH"]
    .filter((detail) => typeof detail === "string")
    .join(" · ");
}

/** The language a file tags as `code`, such as "English" for `eng`, if it tags one. */
function languageName(code: string | undefined) {
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
