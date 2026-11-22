import { useHomeLayoutChanges } from "@/features/settings/hooks/use-home-layout-changes";
import type { HomeLayout } from "@/gen/nama/v1/server_pb";
import { LayoutList } from "@/shared/components/home-layout/layout-list";
import { refusal } from "@/shared/lib/forms";

/**
 * Featured and each Home row of the `saved` layout, with a switch that saves it shown or hidden,
 * and each row dragged or moved by its arrows into the User's own order.
 */
export function HomeLayoutRows({ saved }: { saved: HomeLayout }) {
  const { error, ...changes } = useHomeLayoutChanges(saved);
  return (
    <LayoutList
      changes={changes}
      refused={error ? refusal(error, "save your Home", {}).form : undefined}
    />
  );
}
