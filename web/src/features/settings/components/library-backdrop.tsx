import { SettingGroup } from "@/features/settings/components/setting-group";
import type { Library } from "@/gen/nama/v1/library_pb";
import { ImageChoice } from "@/shared/components/image-choice/image-choice";
import { imageToEditOf } from "@/shared/lib/server-image";

/** The Backdrop `library`'s own page shows it by, to upload one in its place or remove it. */
export function LibraryBackdrop({ library }: { library: Library }) {
  return (
    <SettingGroup title="On the Library’s page">
      <p className="mt-1 mb-4 max-w-sm text-sm text-pretty text-muted-foreground">
        Everyone sees the Library by this Backdrop. Until you upload one, it’s the Backdrop of the
        title added last. JPEG, PNG or WebP, up to 20 MB.
      </p>
      <div className="max-w-sm">
        <ImageChoice
          image={{
            name: "Backdrop",
            shown: imageToEditOf(library.backdropUrl),
            custom: library.customBackdrop,
            path: `/custom-images/library/${String(library.id)}/backdrop`,
            shape: "backdrop",
          }}
        />
      </div>
    </SettingGroup>
  );
}
