import { useLocation, useNavigate, useRouter } from "@tanstack/react-router";
import { ChevronLeft } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { allSections, sectionSearch } from "@/features/settings/lib/sections";

/**
 * A phone's "Settings" at the top of a section, back to the list of sections: Back, when the list
 * opened it, else the list in its place, as when a link opened Settings on the section.
 */
export function BackToSections() {
  const router = useRouter();
  const navigate = useNavigate();
  const fromList = useLocation({ select: ({ state }) => state.fromSettingsList === true });
  return (
    // Hung by the button's own padding, so its chevron lines up with the rows below.
    <span className="-ms-2.5 flex">
      <Button
        variant="ghost"
        onClick={() => {
          if (fromList) {
            router.history.back();
            return;
          }
          void navigate({
            to: ".",
            search: (rest) => ({ ...rest, ...sectionSearch(allSections) }),
            replace: true,
          });
        }}
      >
        <ChevronLeft aria-hidden strokeWidth={1.5} className="rtl:rotate-180" />
        Settings
      </Button>
    </span>
  );
}
