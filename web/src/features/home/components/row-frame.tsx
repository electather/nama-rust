import type { LinkOptions } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { HeadingLink } from "@/features/home/components/heading-link";
import { HomeSection } from "@/features/home/components/home-section";
import { SectionHeading } from "@/features/home/components/section-heading";

/**
 * A row, such as a Plugin's, under its heading, `heading`, which `headingId` names it by and which
 * opens `link` if it has one.
 */
export function RowFrame({
  heading,
  headingId,
  link,
  children,
}: {
  heading: string;
  headingId: string;
  link?: LinkOptions | undefined;
  children: ReactNode;
}) {
  return (
    <HomeSection
      labelledBy={headingId}
      heading={
        <SectionHeading id={headingId}>
          {link ? <HeadingLink link={link}>{heading}</HeadingLink> : heading}
        </SectionHeading>
      }
    >
      {children}
    </HomeSection>
  );
}
