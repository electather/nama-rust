import { expect, test } from "vite-plus/test";

import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { isCovered } from "@/shared/lib/permissions";

// ADR 0020: Admin covers every other Permission, and a parent its children.
test.each([
  { held: [Permission.ADMIN], permission: Permission.MANAGE_USERS, covered: true },
  { held: [Permission.ADMIN], permission: Permission.ADMIN, covered: false },
  { held: [Permission.AUTO_APPROVE], permission: Permission.AUTO_APPROVE_TV, covered: true },
  { held: [Permission.AUTO_APPROVE_TV], permission: Permission.AUTO_APPROVE, covered: false },
  { held: [Permission.MANAGE_USERS], permission: Permission.MANAGE_LIBRARIES, covered: false },
])("$held covers $permission: $covered", ({ held, permission, covered }) => {
  expect(isCovered(held, permission)).toBe(covered);
});
