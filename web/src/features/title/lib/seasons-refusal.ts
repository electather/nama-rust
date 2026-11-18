import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { askRefusal, showHasQuality } from "@/shared/lib/forms";

/** What to tell the User when Requesting Seasons of the Show titled `title` was refused with `error`. */
export function seasonsRefusal(error: unknown, title: string) {
  return askRefusal(error, `Request Seasons of ${title}`, {
    [RefusalReason.FIELD_REQUIRED]: { form: "Pick at least one Season." },
    // A scan Matched one since the page was opened, which reading the Seasons back shows.
    [RefusalReason.ALREADY_IN_LIBRARY]: { form: "One of those Seasons is already in a Library." },
    // Removed from the list since the page opened.
    [RefusalReason.QUALITY_NOT_OFFERED]: {
      form: "That Quality isn’t offered any more. Reload the page and pick another.",
    },
    // Another Season gave the Show its Quality since the page opened.
    [RefusalReason.SHOW_HAS_QUALITY]: {
      form: `${showHasQuality(error, title)} Reload the page to ask at it.`,
    },
  });
}

/** What Requesting the Show titled `title` asks for, the Whole Show or Seasons, in words. */
export function askedFor(title: string, whole: boolean) {
  return whole ? `the Whole Show of ${title}` : `Seasons of ${title}`;
}
