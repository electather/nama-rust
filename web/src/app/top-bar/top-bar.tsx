import type { ComponentProps } from "react";

import { TopBarRow } from "@/app/top-bar/top-bar-row";

/**
 * The bar above every signed-in page: where to go, the action bar's button and your menu. Flush
 * with the page at rest, it lifts into a tray once the page scrolls under it. Its height never
 * changes, so lifting moves nothing below it.
 */
export function TopBar(props: ComponentProps<typeof TopBarRow>) {
  return (
    <header className="top-bar-stick z-20 flex h-18 justify-center">
      <div className="max-w-350 min-w-0 flex-1 outline -outline-offset-1 outline-transparent transition-all duration-300 ease-out-expo motion-reduce:transition-none raised:mx-5 raised:mt-3 raised:rounded-3xl raised:bg-secondary raised:p-1 raised:shadow-lg raised:outline-border raised:backdrop-blur-lg">
        <TopBarRow {...props} />
      </div>
    </header>
  );
}
