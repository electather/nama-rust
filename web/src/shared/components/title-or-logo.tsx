import { ServerImg } from "@/shared/components/server-img";
import type { ServerImage } from "@/shared/lib/server-image";

/**
 * A title as its Title Logo, named by the title, or as text without one or once it fails to load.
 * `look` sizes the logo's box and says how wide it shows, for its `srcset`.
 */
export function TitleOrLogo({
  title,
  logo,
  look,
}: {
  title: string;
  logo: ServerImage | undefined;
  look: { className: string; sizes: string };
}) {
  return (
    <ServerImg
      images={[logo]}
      sizes={look.sizes}
      alt={title}
      className={`w-auto object-contain object-left ${look.className}`}
      fallback={title}
    />
  );
}
