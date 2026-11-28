import { FileQuestionMark, Library } from "lucide-react";

import { LibrariesPane } from "@/features/settings/components/libraries-pane";
import { LibraryCrumbs } from "@/features/settings/components/library-crumbs";
import { NeedsAMatchCrumbs } from "@/features/settings/components/needs-a-match-crumbs";
import { NeedsAMatchPane } from "@/features/settings/components/needs-a-match-pane";
import { NewLibraryLink } from "@/features/settings/components/new-library-link";
import type { SectionGroupDefinition } from "@/features/settings/lib/section-definition";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

/** The Server's Sections of its Libraries. */
export const media = {
  label: "Media",
  sections: {
    libraries: {
      title: "Libraries",
      words:
        "libraries library folders scan movies shows title sources recognisers segment sources backdrop",
      permission: Permission.MANAGE_LIBRARIES,
      icon: Library,
      couldntLoad: "the Libraries",
      labels: {},
      pane: LibrariesPane,
      crumbs: LibraryCrumbs,
      action: NewLibraryLink,
    },
    "needs-a-match": {
      title: "Needs a Match",
      words: "needs a match unmatched identify fix wrong title",
      permission: Permission.MANAGE_LIBRARIES,
      icon: FileQuestionMark,
      couldntLoad: "what needs a Match",
      labels: {},
      pane: NeedsAMatchPane,
      crumbs: NeedsAMatchCrumbs,
    },
  },
} satisfies SectionGroupDefinition;
