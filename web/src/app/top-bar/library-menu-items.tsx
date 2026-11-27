import { Link } from "@tanstack/react-router";
import {
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuLinkItem,
} from "nama-ui/components/dropdown-menu";

import type { LibraryGroup } from "@/app/data/use-top-bar-libraries";

/**
 * A menu's link to each of `group`'s Libraries, on its Linked Server at the Hub, under the
 * Server's name when `named`.
 */
export function LibraryMenuItems({ group, named }: { group: LibraryGroup; named: boolean }) {
  const { server, libraries } = group;
  const items = libraries.map(({ id, name }) => {
    const link = (
      <Link
        to="/libraries/$libraryId"
        params={{ libraryId: String(id) }}
        search={{ server: server?.id }}
      />
    );
    return (
      <DropdownMenuLinkItem key={String(id)} render={link}>
        {name}
      </DropdownMenuLinkItem>
    );
  });
  if (!named) {
    return items;
  }
  return (
    libraries.length > 0 && (
      <DropdownMenuGroup>
        <DropdownMenuLabel>{server?.name}</DropdownMenuLabel>
        {items}
      </DropdownMenuGroup>
    )
  );
}
