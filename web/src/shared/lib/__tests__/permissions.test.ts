import { expect, test } from "vite-plus/test";

import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { isCovered } from "@/shared/lib/permissions";

// ADR 0020: Admin covers every other Permission, and a parent its children.
test.each([
  {
    case: "Admin covers Manage Users",
    held: [Permission.ADMIN],
    permission: Permission.MANAGE_USERS,
    covered: true,
  },
  {
    case: "Admin doesn't cover itself",
    held: [Permission.ADMIN],
    permission: Permission.ADMIN,
    covered: false,
  },
  {
    case: "Auto-approve covers Auto-approve Shows",
    held: [Permission.AUTO_APPROVE],
    permission: Permission.AUTO_APPROVE_TV,
    covered: true,
  },
  {
    case: "a child doesn't cover its parent",
    held: [Permission.AUTO_APPROVE_TV],
    permission: Permission.AUTO_APPROVE,
    covered: false,
  },
  {
    case: "Manage Users doesn't cover Manage Libraries",
    held: [Permission.MANAGE_USERS],
    permission: Permission.MANAGE_LIBRARIES,
    covered: false,
  },
])("$case", ({ held, permission, covered }) => {
  expect(isCovered(held, permission)).toBe(covered);
});
