import { Button } from "nama-ui/components/button";
import { useRef } from "react";

import type { useConnectionChanges } from "@/features/settings/hooks/use-connection-changes";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import { refusal } from "@/shared/lib/forms";
import type { Opener } from "@/shared/lib/opener";

/** The button that disconnects the User from the Plugin `name`, asking first. */
export function Disconnect({
  pluginId,
  name,
  remove,
}: {
  pluginId: string;
  name: string;
  remove: ReturnType<typeof useConnectionChanges>["remove"];
}) {
  const asking = useRef<Opener>(null);
  return (
    <>
      <Button variant="secondary" aria-haspopup="dialog" onClick={() => asking.current?.open()}>
        Disconnect
      </Button>
      <ConfirmDialog
        ref={asking}
        trigger={false}
        destructive={false}
        name={`Disconnect ${name}`}
        action="Disconnect"
        title={`Disconnect ${name}?`}
        description={`${name} stops messaging you about your Requests. You can connect it again at any time.`}
        onConfirm={async () => {
          await remove.mutateAsync({ pluginId });
        }}
        refusal={(error) => refusal(error, `disconnect ${name}`, {}).form}
      />
    </>
  );
}
