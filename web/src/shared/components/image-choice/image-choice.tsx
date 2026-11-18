import { ChangeImage } from "@/shared/components/change-image/change-image";
import { Preview } from "@/shared/components/image-choice/preview";
import type { Choices } from "@/shared/lib/artwork-shapes";
import type { EditableImage } from "@/shared/lib/editable-image";
import type { ImageRules } from "@/shared/lib/image-rules";

/**
 * A title's Poster or Backdrop, which Browse sends, and the Libraries too, since a title's Backdrop
 * can be its Library's.
 */
const titleImages: ImageRules = {
  accept: "image/jpeg,image/png,image/webp",
  refusals: {
    403: "This needs the Manage Libraries Permission.",
    413: "That file is over 20 MB. Pick a smaller one.",
    415: "That isn’t a JPEG, PNG or WebP image.",
    422: "That image is over 12 megapixels. Pick a smaller one.",
    404: "That image isn’t offered any more. Close Edit images and open it again.",
    503: "The title source couldn’t be reached. Try again later.",
  },
  queryKey: [],
};

/** One image, with a way to change it, and `choices` to pick from if a title source lists any. */
export function ImageChoice({
  image,
  choices,
}: {
  image: EditableImage;
  choices?: Choices | undefined;
}) {
  return (
    <figure className="flex items-start gap-4">
      <Preview image={image} />
      <div className="min-w-0 flex-1">
        <figcaption className="text-sm font-medium">{image.name}</figcaption>
        {image.custom && <p className="text-sm text-muted-foreground">Custom image</p>}
        <ChangeImage image={image} rules={titleImages} choices={choices} />
      </div>
    </figure>
  );
}
