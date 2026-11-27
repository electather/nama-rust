import { create } from "@bufbuild/protobuf";
import { describe, expect, test } from "vite-plus/test";

import { type SectionGroupDefinition, everyUser } from "@/features/settings/lib/section-definition";
import { activity } from "@/features/settings/lib/section-groups/activity";
import { admin } from "@/features/settings/lib/section-groups/admin";
import { media } from "@/features/settings/lib/section-groups/media";
import { yourAccount } from "@/features/settings/lib/section-groups/your-account";
import { definitions, groupsFor } from "@/features/settings/lib/sections";
import { Permission, UserSchema } from "@/gen/nama/v1/sign_in_pb";
import { sections } from "@/shared/lib/settings-address";

describe("permissions", () => {
  const permitted: SectionGroupDefinition[] = [media, activity, admin];
  const account: SectionGroupDefinition = yourAccount;

  test.each(permitted)("every Section in $label names a Permission", (group) => {
    const unnamed = Object.values(group.sections).filter(
      (definition) => definition.permission === everyUser,
    );

    expect(unnamed.map((definition) => definition.title)).toStrictEqual([]);
  });

  test("no Section of the Account names a Permission", () => {
    const named = Object.values(account.sections).filter(
      (definition) => definition.permission !== everyUser,
    );

    expect(named.map((definition) => definition.title)).toStrictEqual([]);
  });

  test("a User with no Permission is shown only You and Preferences", () => {
    const shown = groupsFor(create(UserSchema), false);

    expect(shown.map((group) => group.label)).toStrictEqual(["You", "Preferences"]);
  });
});

describe("groups", () => {
  const atServer = groupsFor(create(UserSchema, { permissions: [Permission.ADMIN] }), false);
  const atHub = groupsFor(undefined, true);

  test("an Admin's Server lists each of its Sections under its group, in order", () => {
    expect(atServer).toStrictEqual([
      { label: "You", items: ["signin", "devices"] },
      { label: "Preferences", items: ["playback", "home", "requests", "connections"] },
      { label: "Media", items: ["libraries", "needs-a-match"] },
      { label: "Activity", items: ["now-playing", "jobs"] },
      {
        label: "Admin",
        items: ["server", "users", "backups", "default-home", "server-playback", "plugins"],
      },
    ]);
  });

  test("the Hub lists only the Account's Sections, in order", () => {
    expect(atHub).toStrictEqual([
      {
        label: "Your Account",
        items: [
          "account",
          "account-signin",
          "account-sessions",
          "account-servers",
          "account-history",
        ],
      },
    ]);
  });

  test("every Section sits in exactly one group", () => {
    const listed = [...atServer, ...atHub].flatMap((group) => group.items);

    expect(new Set(listed)).toStrictEqual(new Set(sections));
    expect(listed).toHaveLength(sections.length);
  });
});

test("a row shown only sometimes is shown by a Setting of its own Section", () => {
  const strays = Object.values(definitions).flatMap(({ labels }) =>
    Object.values(labels)
      .map((text) => text.shownBy)
      .filter((key) => key !== undefined)
      .filter((key) => !Object.hasOwn(labels, key)),
  );

  expect(strays).toStrictEqual([]);
});
