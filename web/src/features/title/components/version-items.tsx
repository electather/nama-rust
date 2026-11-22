import {
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuSeparator,
} from "nama-ui/components/dropdown-menu";

import { VersionRadios } from "@/features/title/components/version-radios";
import type { Version } from "@/gen/nama/v1/browse_pb";

/** The choice of which of `versions` Play plays, `version` checked, atop a title's menu. */
export function VersionItems(props: {
  versions: Version[];
  version: Version;
  onChoose: (versionId: number) => void;
}) {
  return (
    <>
      <DropdownMenuGroup>
        <DropdownMenuLabel>Version</DropdownMenuLabel>
        <VersionRadios {...props} />
      </DropdownMenuGroup>
      <DropdownMenuSeparator />
    </>
  );
}
