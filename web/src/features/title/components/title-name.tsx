import { TitleOrLogo } from "@/shared/components/title-or-logo";
import type { ServerImage } from "@/shared/lib/server-image";

const pageLogo = {
  className: "max-h-28 max-w-4/5 sm:max-h-36 sm:max-w-md",
  sizes: "(min-width: 640px) 448px, 80vw",
};

/** The page's heading: the Title Logo, or else the title as text. */
export function TitleName({ title, logo }: { title: string; logo: ServerImage | undefined }) {
  return (
    <h1 className="text-display font-medium text-balance">
      <TitleOrLogo title={title} logo={logo} look={pageLogo} />
    </h1>
  );
}
