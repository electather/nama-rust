import { PowerOff } from "lucide-react";
import { useRef, useState } from "react";

import { RequestedPermissions } from "@/features/admin-plugins/components/requested-permissions";
import { useEnablePlugin } from "@/features/admin-plugins/data/use-enable-plugin";
import { refusedChange } from "@/features/admin-plugins/lib/refused-change";
import type { PluginSummary, Setting } from "@/gen/nama/v1/plugin_pb";
import { ConfirmModal } from "@/shared/components/confirm-dialog/confirm-modal";
import { StateLabel } from "@/shared/components/state-label";
import { pluginName } from "@/shared/lib/plugins";
import { Button } from "@/shared/ui/button";

/**
 * A disabled Plugin that requests Plugin Permissions, with an Enable button that opens a dialog
 * with one checkbox for each, none checked, and enables it granting only those checked. The
 * dialog says why the Server refused. As a view's `primary` button it stands alone, with no state
 * beside it.
 */
export function GrantAndEnable({
  summary,
  settings,
  primary = false,
}: {
  summary: PluginSummary;
  settings: Setting[] | undefined;
  primary?: boolean;
}) {
  const enable = useEnablePlugin();
  const name = pluginName(summary);
  const dialog = useRef<HTMLDialogElement>(null);
  const [openings, setOpenings] = useState(0);
  return (
    <div className="flex items-center gap-2">
      {!primary && <StateLabel label="Disabled" Icon={PowerOff} />}
      <Button
        variant={primary ? "default" : "ghost"}
        aria-label={`Enable ${name}`}
        aria-haspopup="dialog"
        onClick={() => {
          setOpenings(openings + 1);
          dialog.current?.showModal();
        }}
      >
        Enable
      </Button>
      <ConfirmModal
        ref={dialog}
        openings={openings}
        action="Enable"
        title={`Enable ${name}?`}
        description="It asks to do these in the Server. It gets only the ones you check. To change them later, disable it and enable it again."
        onConfirm={async (typed) => {
          const checked = typed.getAll("permission");
          const grantedPermissions = summary.requestedPermissions.filter((permission) =>
            checked.includes(String(permission)),
          );
          await enable.mutateAsync({ id: summary.id, grantedPermissions });
        }}
        refusal={(error) => refusedChange(error, `enable ${name}`, settings)}
      >
        <RequestedPermissions requested={summary.requestedPermissions} />
      </ConfirmModal>
    </div>
  );
}
