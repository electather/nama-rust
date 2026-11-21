import { create } from "@bufbuild/protobuf";

import { type Track, TrackSchema } from "@/gen/nama/v1/browse_pb";
import { languageName } from "@/shared/lib/language-name";

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
  const facts = [named && title, forced && "Forced", sdh && "SDH"].filter(
    (detail) => typeof detail === "string",
  );
  // A file often titles its SDH Track "SDH", too.
  return [...new Set(facts)].join(" · ");
}
