import type { ReactNode } from "react";

/** A heading of a title's Details, such as "Languages", and its facts by name. */
export interface DetailGroup {
  heading: string;
  /** Each fact's name and value; one without a value is left out. */
  facts: [string, ReactNode][];
}
