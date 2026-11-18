import { useArtwork } from "@/features/title/data/use-artwork";
import { ImageChoice } from "@/shared/components/image-choice/image-choice";
import type { EditableImage } from "@/shared/lib/editable-image";

/** One image, with the artwork `artwork` lists offer in its place once they have arrived. */
export function ListedImageChoice({
  image,
  artwork: { asked, list },
}: {
  image: EditableImage;
  artwork: NonNullable<EditableImage["artwork"]>;
}) {
  // Images asking the same lists share one call.
  const listed = useArtwork(asked);
  const choices = listed && { urls: listed[list], shape: image.shape };
  return <ImageChoice image={image} choices={choices} />;
}
