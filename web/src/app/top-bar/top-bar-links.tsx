import { TopBarLink } from "@/app/top-bar/top-bar-link";

/**
 * The top bar's links. A phone has no room for them: there the action bar's "Jump to" goes to
 * each.
 */
export function TopBarLinks() {
  return (
    <nav aria-label="Main" className="hidden items-center gap-0.5 sm:flex">
      <TopBarLink to="/" activeOptions={{ exact: true }}>
        Home
      </TopBarLink>
    </nav>
  );
}
