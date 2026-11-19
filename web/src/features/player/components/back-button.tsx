import type { LinkOptions } from "@tanstack/react-router";
import { ChevronLeft } from "lucide-react";

import { BackLink } from "@/features/player/components/back-link";

/** The title bar's button back to the title page. */
export function BackButton({ back }: { back: LinkOptions }) {
  return (
    <BackLink back={back} variant="secondary" size="round" label="Back">
      <ChevronLeft aria-hidden />
    </BackLink>
  );
}
