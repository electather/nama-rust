import { create } from "@bufbuild/protobuf";

import { ResizeField } from "@/features/settings/components/resize-field";
import { useImageSettings } from "@/features/settings/data/use-image-settings";
import { useUpdateImageSettings } from "@/features/settings/data/use-update-image-settings";
import { imageSettingsErrors, typedSettings } from "@/features/settings/lib/image-settings";
import { ImageSettingsSchema } from "@/gen/nama/v1/server_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { FormField } from "@/shared/components/form-field/form-field";
import { onSubmitted } from "@/shared/lib/forms";

/**
 * The form a User who manages settings sets how Posters and Backdrops are resized with: whether
 * they are, and how much disk the resized copies take.
 */
export function ImageSettingsForm() {
  const settings =
    useImageSettings().settings ?? create(ImageSettingsSchema, { resizeImages: true });
  const update = useUpdateImageSettings();
  const errors = imageSettingsErrors(update.error);

  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        update.mutate({ settings: typedSettings(typed) });
      })}
    >
      <ResizeField saved={settings.resizeImages} />
      <FormField
        label="Space for resized images"
        name="resizedCacheMib"
        type="number"
        min={1}
        required={false}
        defaultValue={settings.resizedCacheMib}
        hint="The most disk, in MiB, the resized copies take. Past it, those shown least recently are deleted, and made again when they’re next shown. Leave it empty for 1024 MiB."
        error={errors.resizedCacheMib}
      />
      <FormEnd
        action={update.isPending ? "Saving image settings…" : "Save image settings"}
        variant="ghost"
        pending={update.isPending}
        done={update.isSuccess ? "Saved." : undefined}
        refusal={errors.form}
      />
    </form>
  );
}
