import { ImageSettingsForm } from "@/features/settings/components/image-settings-form";
import { ImageStore } from "@/features/settings/components/image-store";

/** The Images heading over the resizing settings and the image store. */
export function ImagesSection() {
  return (
    <section aria-labelledby="images" className="mt-12 max-w-md">
      <h2 id="images" className="font-medium">
        Images
      </h2>
      <ImageSettingsForm />
      <ImageStore />
    </section>
  );
}
