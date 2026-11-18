import { useId } from "react";

import { ArtworkChoice } from "@/shared/components/artwork-choices/artwork-choice";
import type { Choices } from "@/shared/lib/artwork-shapes";

/**
 * The artwork the title sources list for the image `name`, each a button that picks it; nothing
 * if they list none.
 */
export function ArtworkChoices({
  name,
  choices: { urls, shape },
  pending,
  onPicked,
}: {
  name: string;
  choices: Choices;
  pending: boolean;
  onPicked: (url: string) => void;
}) {
  const id = useId();
  if (urls.length === 0) {
    return null;
  }
  return (
    <>
      <p id={id} className="mt-2 text-sm text-muted-foreground">
        Or pick one from the title sources
      </p>
      {/* Scrolls sideways, so a long list keeps the dialog short. */}
      <ul aria-labelledby={id} className="mt-2 flex gap-2 overflow-x-auto p-1">
        {urls.map((url, index) => (
          <ArtworkChoice
            key={url}
            name={name}
            url={url}
            index={index}
            of={urls.length}
            shape={shape}
            pending={pending}
            onPicked={onPicked}
          />
        ))}
      </ul>
    </>
  );
}
