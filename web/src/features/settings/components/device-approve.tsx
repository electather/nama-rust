import type { ConnectError } from "@connectrpc/connect";

import { useApproveDevice } from "@/features/settings/data/use-approve-device";
import { Button } from "@/shared/ui/button";

/**
 * The second step of signing in another Device from the Devices pane: names the Device `code`
 * belongs to, so it's signed in only once the User has seen which.
 */
export function DeviceApprove({
  code,
  deviceName,
  onApproved,
  onRefused,
  onCancel,
}: {
  code: string;
  deviceName: string;
  onApproved: () => void;
  onRefused: (error: ConnectError) => void;
  onCancel: () => void;
}) {
  const approve = useApproveDevice(deviceName, onApproved, onRefused);
  const buttons = (
    <div className="flex gap-1">
      <Button
        disabled={approve.isPending}
        onClick={() => {
          approve.mutate({ code });
        }}
      >
        {approve.isPending ? "Signing it in…" : "Sign it in"}
      </Button>
      <Button variant="ghost" onClick={onCancel}>
        Cancel
      </Button>
    </div>
  );
  return (
    <div className="flex flex-col gap-3 py-2.5">
      <div className="flex flex-col gap-0.5">
        <p className="text-sm font-medium">Sign in “{deviceName}”?</p>
        <p className="text-xs text-pretty text-muted-foreground">
          It’s asking to sign in as you. Sign it in only if it’s yours.
        </p>
      </div>
      {buttons}
    </div>
  );
}
