import { ListedImageChoice } from "@/features/title/components/listed-image-choice";
import type { TitleImages } from "@/features/title/lib/title-images";
import { ImageChoice } from "@/shared/components/image-choice/image-choice";

/** Each of `images`, with the artwork its `artwork` lists offer in its place. */
export function ImageChoices({ images }: TitleImages) {
  return (
    <div className="mt-6 flex flex-col gap-5">
      {images.map(({ artwork, ...image }) =>
        artwork === undefined ? (
          <ImageChoice key={image.path} image={image} />
        ) : (
          <ListedImageChoice key={image.path} image={image} artwork={artwork} />
        ),
      )}
    </div>
  );
}
