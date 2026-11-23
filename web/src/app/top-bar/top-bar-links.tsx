import { TopBarLink } from "@/app/top-bar/top-bar-link";
import { useLoadedLibraries } from "@/shared/hooks/use-libraries-query";

/**
 * The top bar's links: Home, then each Library. A phone has no room for them: there the action
 * bar goes to each, and Home lists the Libraries.
 */
export function TopBarLinks() {
  const libraries = useLoadedLibraries();
  return (
    // Scrolls sideways rather than push search off a narrow bar; the padding keeps focus rings in.
    <nav
      aria-label="Main"
      className="-m-1 hidden min-w-0 scrollbar-none items-center gap-0.5 overflow-x-auto p-1 sm:flex"
    >
      <TopBarLink to="/" activeOptions={{ exact: true }}>
        Home
      </TopBarLink>
      {libraries.map(({ id, name }) => (
        <TopBarLink key={String(id)} to="/libraries/$libraryId" params={{ libraryId: String(id) }}>
          {name}
        </TopBarLink>
      ))}
    </nav>
  );
}
