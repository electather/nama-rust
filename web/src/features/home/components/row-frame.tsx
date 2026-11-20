import type { ReactNode } from "react";

import { HomeSection } from "@/features/home/components/home-section";
import { SectionHeading } from "@/features/home/components/section-heading";

/** A Plugin's row under its heading, `heading`, which `headingId` names it by. */
export function RowFrame({
  heading,
  headingId,
  children,
}: {
  heading: string;
  headingId: string;
  children: ReactNode;
}) {
  return (
    <HomeSection
      labelledBy={headingId}
      heading={<SectionHeading id={headingId}>{heading}</SectionHeading>}
    >
      {children}
    </HomeSection>
  );
}
