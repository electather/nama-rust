import { Shield, Tv } from "lucide-react";

import { DevicesPane } from "@/features/settings/components/devices-pane";
import { SigninPane } from "@/features/settings/components/signin-pane";
import { deviceLabels, signinLabels } from "@/features/settings/lib/pane-labels";
import { type SectionGroupDefinition, everyUser } from "@/features/settings/lib/section-definition";

/** The Server's Sections about the User themselves. */
export const you = {
  label: "You",
  sections: {
    signin: {
      title: "Sign-in",
      permission: everyUser,
      icon: Shield,
      couldntLoad: "your sign-in details",
      labels: signinLabels,
      pane: SigninPane,
    },
    devices: {
      title: "Devices",
      permission: everyUser,
      icon: Tv,
      couldntLoad: "your Devices",
      labels: deviceLabels,
      pane: DevicesPane,
    },
  },
} satisfies SectionGroupDefinition;
