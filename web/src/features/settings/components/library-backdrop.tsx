import { SettingGroup } from "@/features/settings/components/setting-group";
import type { Library } from "@/gen/nama/v1/library_pb";
import { ImageChoice } from "@/shared/components/image-choice/image-choice";

/** The Backdrop the home page shows `library` by, to upload one in its place or remove it. */
export function LibraryBackdrop({ library }: { library: Library }) {
  return (
    <SettingGroup title="On the home page">
      <p className="mt-1 mb-4 max-w-sm text-sm text-pretty text-muted-foreground">
        Everyone sees the Library by this Backdrop. Until you upload one, it’s the Backdrop of the
        title added last. JPEG, PNG or WebP, up to 20 MB.
      </p>
      <div className="max-w-sm">
        <ImageChoice
          image={{
            name: "Backdrop",
            url: library.backdropUrl,
            custom: library.customBackdrop,
            path: `/custom-images/library/${String(library.id)}/backdrop`,
            shape: "backdrop",
          }}
        />
      </div>
    </SettingGroup>
  );
}
