import { usePluginNames } from "@/features/admin-home/data/use-plugin-names";
import { useDefaultHomeChanges } from "@/features/admin-home/hooks/use-default-home-changes";
import { LayoutList } from "@/shared/components/home-layout/layout-list";
import { refusal } from "@/shared/lib/forms";

/** The default Home layout as an Admin changes it, each change saved as it's made. */
export function DefaultHomeRows() {
  const names = usePluginNames();
  const { error, ...changes } = useDefaultHomeChanges(names);
  return (
    <LayoutList
      changes={changes}
      names={names}
      refused={error ? refusal(error, "save the default Home layout", {}).form : undefined}
    />
  );
}
