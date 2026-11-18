import type { ReactNode } from "react";

import { TitleBackdrop } from "@/features/title/components/title-backdrop";
import { TitleHeading } from "@/features/title/components/title-heading";
import { TitleHero } from "@/features/title/components/title-hero";

/**
 * A Movie's or Show's page: its Title Logo, or else its title as text, over its backdrop, then
 * `children`. The backdrop sits under the scrim, so the title keeps its contrast whatever the
 * image (DESIGN.md, "The Legible Over Art Rule"). Only the title goes over the art: `muted-foreground`
 * wouldn't keep 4.5:1 there. Its `backdropThumbhash` shows until the backdrop loads.
 */
export function TitlePage({
  title,
  logoUrl,
  backdropUrl,
  backdropThumbhash,
  children,
}: {
  title: string;
  logoUrl?: string | undefined;
  backdropUrl: string | undefined;
  backdropThumbhash: Uint8Array | undefined;
  children: ReactNode;
}) {
  const backdrop = <TitleBackdrop url={backdropUrl} thumbhash={backdropThumbhash} />;
  const heading = <TitleHeading title={title} logoUrl={logoUrl} />;
  return (
    <main>
      <TitleHero art={backdropUrl !== undefined} backdrop={backdrop} heading={heading} />
      <div className="mx-auto max-w-5xl px-4 pb-16 sm:px-6">{children}</div>
    </main>
  );
}
