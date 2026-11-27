import { useLocation, useNavigate, useRouter } from "@tanstack/react-router";

import { allSections, sectionSearch } from "@/features/settings/lib/sections";

/**
 * A phone's "Settings" crumb over a section's page, back to the list of sections: Back, when the
 * list opened it, else the list in its place, as when a link opened Settings on the section. It
 * looks like the list crumbs after it (`ListCrumb`).
 */
export function SettingsCrumb() {
  const router = useRouter();
  const navigate = useNavigate();
  const fromList = useLocation({ select: ({ state }) => state.fromSettingsList === true });
  return (
    <button
      type="button"
      className="cursor-pointer rounded-sm text-muted-foreground outline-none hover:text-foreground hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
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
      Settings
    </button>
  );
}
