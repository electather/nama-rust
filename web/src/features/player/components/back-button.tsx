import type { LinkOptions } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";

import { BackLink } from "@/features/player/components/back-link";

/** The title bar's button back to the title page. */
export function BackButton({ back }: { back: LinkOptions }) {
  return (
    <BackLink back={back} variant="overlay" size="icon" label="Back">
      <ArrowLeft aria-hidden />
    </BackLink>
  );
}
