import { Link } from "@tanstack/react-router";
import { Plus } from "lucide-react";

import { DeviceItem } from "@/features/settings/components/device-item";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { useYourDevices } from "@/features/settings/data/use-your-devices";
import { buttonVariants } from "@/shared/ui/button";

/**
 * Where the signed-in User is signed in: each Device, with a button to revoke it unless it's this
 * one, and the way to sign in another.
 */
export function DevicesPane() {
  const devices = useYourDevices();
  const signIn = (
    <Link to="/code" className={buttonVariants()}>
      <Plus aria-hidden strokeWidth={1.5} />
      Sign in a Device
    </Link>
  );
  return (
    <PaneFrame section="devices" action={signIn}>
      <ul aria-label="Your Devices" className="-my-3.5 flex flex-col">
        {devices.map((device) => (
          <DeviceItem key={device.id} device={device} />
        ))}
      </ul>
    </PaneFrame>
  );
}
