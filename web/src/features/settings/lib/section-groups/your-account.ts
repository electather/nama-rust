import { CircleUser, History, KeyRound, Laptop, Server } from "lucide-react";

import { AccountHistoryPane } from "@/features/settings/components/account-history-pane";
import { AccountPane } from "@/features/settings/components/account-pane";
import { AccountServersPane } from "@/features/settings/components/account-servers-pane";
import { AccountSessionsPane } from "@/features/settings/components/account-sessions-pane";
import { AccountSigninPane } from "@/features/settings/components/account-signin-pane";
import {
  accountHistoryLabels,
  accountLabels,
  accountServerLabels,
  accountSessionLabels,
  accountSigninLabels,
} from "@/features/settings/lib/pane-labels";
import { type SectionGroupDefinition, everyUser } from "@/features/settings/lib/section-definition";

/** The Account's own Sections, shown only at the Hub (ADR 0105), where no Server's are. */
export const yourAccount = {
  label: "Your Account",
  sections: {
    account: {
      title: "Account",
      words: "email delete",
      permission: everyUser,
      icon: CircleUser,
      couldntLoad: "your Account",
      labels: accountLabels,
      pane: AccountPane,
    },
    "account-signin": {
      title: "Sign-in",
      words: "password passkeys",
      permission: everyUser,
      icon: KeyRound,
      couldntLoad: "your sign-in details",
      labels: accountSigninLabels,
      pane: AccountSigninPane,
    },
    "account-sessions": {
      title: "Sessions",
      words: "sign out browsers",
      permission: everyUser,
      icon: Laptop,
      couldntLoad: "where you’re signed in",
      labels: accountSessionLabels,
      pane: AccountSessionsPane,
    },
    "account-servers": {
      title: "Servers",
      words: "linked unlink link code invites accept",
      permission: everyUser,
      icon: Server,
      couldntLoad: "your Servers and Users",
      labels: accountServerLabels,
      pane: AccountServersPane,
    },
    "account-history": {
      title: "Viewing History",
      words: "viewing events plays export pooled counts",
      permission: everyUser,
      icon: History,
      couldntLoad: "your viewing history",
      labels: accountHistoryLabels,
      pane: AccountHistoryPane,
    },
  },
} satisfies SectionGroupDefinition;
