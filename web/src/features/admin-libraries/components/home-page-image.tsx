import type { Library } from "@/gen/nama/v1/library_pb";
import { ImageChoice } from "@/shared/components/image-choice/image-choice";

/** The Backdrop the home page shows the Library by, to upload one in its place or remove it. */
export function HomePageImage({ library }: { library: Library }) {
  return (
    <section aria-labelledby="home-page" className="mt-10 max-w-sm">
      <h2 id="home-page" className="font-medium">
        On the home page
      </h2>
      <p className="mt-1 text-sm text-pretty text-muted-foreground">
        Everyone sees the Library by this Backdrop. Until you upload one, it’s the Backdrop of the
        title added last. JPEG, PNG or WebP, up to 20 MB.
      </p>
      <div className="mt-4">
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
    </section>
  );
}
