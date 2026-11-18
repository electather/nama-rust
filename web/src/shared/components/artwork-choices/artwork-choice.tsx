import { cn } from "cn";

import { ChoiceImage } from "@/shared/components/artwork-choices/choice-image";
import { type Choices, choiceShapes } from "@/shared/lib/artwork-shapes";

/** One artwork `url` of the image `name`, the `index`th of `of`, as a button that picks it. */
export function ArtworkChoice({
  name,
  url,
  index,
  of,
  shape,
  pending,
  onPicked,
}: {
  name: string;
  url: string;
  index: number;
  of: number;
  shape: Choices["shape"];
  pending: boolean;
  onPicked: (url: string) => void;
}) {
  return (
    <li className="shrink-0">
      <button
        type="button"
        aria-label={`Pick ${name} ${String(index + 1)} of ${String(of)}`}
        disabled={pending}
        onClick={() => {
          onPicked(url);
        }}
        className={cn(
          "block overflow-hidden rounded-md border bg-muted outline-none hover:ring-2 hover:ring-foreground/40 focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none",
          choiceShapes[shape].className,
        )}
      >
        <ChoiceImage url={url} sizes={choiceShapes[shape].sizes} />
      </button>
    </li>
  );
}
