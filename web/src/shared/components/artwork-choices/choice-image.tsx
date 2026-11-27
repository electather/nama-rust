import { ServerImg } from "@/shared/components/server-img";
import { artworkOf } from "@/shared/lib/server-image";

/** One artwork to pick, at the smallest copy the Server resizes to that fits `sizes`. */
export function ChoiceImage({ url, sizes }: { url: string; sizes: string }) {
  // The button names it, so the image itself says nothing more.
  return <ServerImg images={[artworkOf(url)]} sizes={sizes} className="size-full object-cover" />;
}
