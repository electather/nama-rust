import { useState } from "react";

import { DeviceItems } from "@/features/settings/components/device-items";
import { OtherActions } from "@/features/settings/components/other-actions";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { SignInDevice } from "@/features/settings/components/sign-in-device";
import type { Device } from "@/gen/nama/v1/sign_in_pb";

/**
 * The User's Devices but this one, `others`, with signing in another over them, here in the pane,
 * and once it's done, which Device was.
 */
export function OtherDevices({ others }: { others: Device[] }) {
  const [adding, setAdding] = useState(false);
  const [signedIn, setSignedIn] = useState<string>();
  const add = () => {
    setSignedIn(undefined);
    setAdding(true);
  };
  const signIn = (
    <SignInDevice
      onSignedIn={(name) => {
        setAdding(false);
        setSignedIn(name);
      }}
      onCancel={() => {
        setAdding(false);
      }}
    />
  );
  return (
    <SettingGroup
      title="Other Devices"
      action={<OtherActions others={others} adding={adding} onAdd={add} />}
    >
      {adding && signIn}
      <output className="py-1 text-sm text-muted-foreground empty:hidden">
        {signedIn === undefined ? "" : `“${signedIn}” is signed in.`}
      </output>
      <DeviceItems label="Other Devices" devices={others} />
      {others.length === 0 && (
        <p className="py-2.5 text-sm text-muted-foreground">You’re signed in nowhere else.</p>
      )}
    </SettingGroup>
  );
}
