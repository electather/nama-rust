import type { ReactNode } from "react";

import { HomeSection } from "@/features/home/components/home-section";
import { UpNextHeading } from "@/features/home/components/up-next-heading";

/** Continue Watching under its heading, which `headingId` names it by. */
export function ContinueWatchingFrame({
  headingId,
  children,
}: {
  headingId: string;
  children: ReactNode;
}) {
  return (
    <HomeSection labelledBy={headingId} heading={<UpNextHeading id={headingId} />}>
      {children}
    </HomeSection>
  );
}
