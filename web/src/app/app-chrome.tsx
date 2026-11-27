import { useMatches } from "@tanstack/react-router";
import type { ComponentProps } from "react";

import { ActionBarHost } from "@/app/top-bar/action-bar-host";
import { TopBar } from "@/app/top-bar/top-bar";

/** The app chrome over a signed-in page, unless the page fills the screen. */
export function AppChrome(props: Omit<ComponentProps<typeof TopBar>, "onSearch">) {
  // A full-screen page that didn't load, such as an unknown Movie's player, keeps the chrome, so
  // its not-found or error page has a way out.
  const fullScreen = useMatches({
    select: (matches) =>
      matches.some((m) => m.staticData.fullScreen === true && m.status === "success"),
  });
  return (
    !fullScreen && (
      <ActionBarHost>{(onSearch) => <TopBar {...props} onSearch={onSearch} />}</ActionBarHost>
    )
  );
}
