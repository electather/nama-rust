import { ArtworkChoices } from "@/shared/components/artwork-choices/artwork-choices";
import { RemoveImage } from "@/shared/components/change-image/remove-image";
import { UploadButton } from "@/shared/components/change-image/upload-button";
import { useChangeImage } from "@/shared/hooks/use-change-image";
import type { Choices } from "@/shared/lib/artwork-shapes";
import type { ImageRules } from "@/shared/lib/image-rules";

/**
 * Upload, a pick of `choices` if there are any, and, for a Custom Image, Remove, for the image
 * `image` names, and why the Server refused any of them.
 */
export function ChangeImage({
  image,
  rules,
  choices,
}: {
  image: {
    /** What it is, such as "Poster". */
    name: string;
    /** Whether it is a Custom Image, which Remove takes away. */
    custom: boolean;
    /** Where its Custom Image is uploaded to and removed from, as the Server names it. */
    path: string;
  };
  rules: ImageRules;
  choices?: Choices | undefined;
}) {
  const { upload, pick, change, pending, error } = useChangeImage(image.path, rules);
  return (
    <>
      <div className="-ms-3 mt-2 flex flex-wrap">
        <UploadButton name={image.name} accept={rules.accept} pending={pending} onPicked={upload} />
        {image.custom && (
          <RemoveImage
            name={image.name}
            pending={pending}
            onRemove={() => {
              void change({ method: "DELETE" });
            }}
          />
        )}
      </div>
      {choices && (
        <ArtworkChoices name={image.name} choices={choices} pending={pending} onPicked={pick} />
      )}
      {error !== undefined && (
        <p role="alert" className="text-sm text-pretty text-destructive">
          {error}
        </p>
      )}
    </>
  );
}
