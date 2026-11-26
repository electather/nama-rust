import { cn } from "cn";

import { useServerUrl } from "@/shared/hooks/use-server-url";
import { type EditableImage, shapes } from "@/shared/lib/editable-image";

/** The image as it is now, in its shape's frame, which is there before it loads. */
export function Preview({ image }: { image: EditableImage }) {
  const src = useServerUrl(image.url);
  return (
    <div className={cn("shrink-0 overflow-hidden rounded-lg border bg-muted", shapes[image.shape])}>
      {image.url !== undefined && (
        <img src={src} alt={image.name} className="size-full object-cover" />
      )}
    </div>
  );
}
