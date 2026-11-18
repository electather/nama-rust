import { cn } from "cn";

import { type EditableImage, shapes } from "@/shared/lib/editable-image";

/** The image as it is now, in its shape's frame, which is there before it loads. */
export function Preview({ image }: { image: EditableImage }) {
  return (
    <div className={cn("shrink-0 overflow-hidden rounded-lg border bg-muted", shapes[image.shape])}>
      {image.url !== undefined && (
        <img src={image.url} alt={image.name} className="size-full object-cover" />
      )}
    </div>
  );
}
