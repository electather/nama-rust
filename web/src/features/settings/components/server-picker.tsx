import { useNavigate } from "@tanstack/react-router";
import { useId } from "react";

import { SelectBox } from "@/features/settings/components/select-box";
import type { AdminServer } from "@/features/settings/data/use-admin-servers";
import { settingsDetailParams } from "@/shared/lib/settings-detail-params";

/**
 * At the Hub, picks whose Settings show: the Account's own, or those of one of `servers`, where its
 * User is an Admin. Shows nothing while it has no such Server.
 */
export function ServerPicker({
  servers,
  picked,
}: {
  servers: AdminServer[];
  picked: AdminServer | undefined;
}) {
  const labelId = useId();
  const navigate = useNavigate();
  if (servers.length === 0) {
    return null;
  }
  const options = [
    { value: "", label: "Your Account" },
    ...servers.map(({ server }) => ({ value: server.id, label: server.name })),
  ];
  return (
    <div className="flex flex-col gap-1 max-md:mx-4 max-md:mb-2">
      <span id={labelId} className="text-xs text-muted-foreground md:px-2">
        Settings for
      </span>
      <SelectBox
        control={{ "aria-labelledby": labelId }}
        options={options}
        value={picked?.server.id ?? ""}
        onChange={(id) => {
          void navigate({
            to: ".",
            search: (rest) => ({
              ...rest,
              ...settingsDetailParams,
              manage: id === "" ? undefined : id,
            }),
            // As a Section's link does, so Back closes Settings.
            replace: true,
          });
        }}
      />
    </div>
  );
}
