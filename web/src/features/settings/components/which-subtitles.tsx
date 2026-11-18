import { useState } from "react";

import { kindOf, kinds, savedKind } from "@/features/settings/lib/track-language";
import { SubtitleKind, type UsersSubtitlePreference } from "@/gen/nama/v1/playback_pb";
import { Choices } from "@/shared/components/form-field/choices";

/**
 * Which subtitles in the language picked a User wants, starting at the ones `saved` wants, and
 * whether only over audio in another language, which forced-only subtitles are for anyway.
 */
export function WhichSubtitles({ saved }: { saved: UsersSubtitlePreference | undefined }) {
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
