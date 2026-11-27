import { cn } from "cn";

import { ServerImg } from "@/shared/components/server-img";
import { type EditableImage, shapes } from "@/shared/lib/editable-image";

/** How wide each shape's frame shows. */
const sizes = { poster: "96px", backdrop: "160px" };

/** The image as it is now, in its shape's frame, which is there before it loads. */
export function Preview({ image }: { image: EditableImage }) {
  return (
    <div className={cn("shrink-0 overflow-hidden rounded-lg border bg-muted", shapes[image.shape])}>
      <ServerImg
        images={[image.shown]}
        sizes={sizes[image.shape]}
        alt={image.name}
        className="size-full object-cover"
      />
    </div>
  );
}
