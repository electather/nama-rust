import { useRef, useState } from "react";
import { flushSync } from "react-dom";

import { DeviceItems } from "@/features/settings/components/device-items";
import { OtherActions } from "@/features/settings/components/other-actions";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { SignInDevice } from "@/features/settings/components/sign-in-device";
import { deviceLabels } from "@/features/settings/lib/pane-labels";
import type { Device } from "@/gen/nama/v1/sign_in_pb";

/**
 * The User's Devices but this one, `others`, with signing in another over them, here in the pane,
 * and once it's done, which Device was.
 */
export function OtherDevices({ others }: { others: Device[] }) {
  const [adding, setAdding] = useState(false);
  const [signedIn, setSignedIn] = useState<string>();
  const addButton = useRef<HTMLButtonElement>(null);
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
        flushSync(() => {
          setAdding(false);
        });
        // The form that had focus is gone, so focus goes back to what opened it, once
        // `flushSync` has put that button back in the page.
        addButton.current?.focus();
      }}
    />
  );
  return (
    <SettingGroup
      setting="others"
      title={deviceLabels.others.label}
      action={<OtherActions others={others} adding={adding} addButton={addButton} onAdd={add} />}
    >
      {adding && signIn}
      <output className="py-1 text-sm text-muted-foreground empty:hidden">
        {signedIn === undefined ? "" : `“${signedIn}” is signed in.`}
      </output>
      <DeviceItems label={deviceLabels.others.label} devices={others} />
      {others.length === 0 && (
        <p className="py-2.5 text-sm text-muted-foreground">You’re signed in nowhere else.</p>
      )}
    </SettingGroup>
  );
}
