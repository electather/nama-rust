import type { Section } from "@/features/settings/lib/sections";

/** What a pane that didn't load says it couldn't load. */
export const loadNouns: Record<Section, string> = {
  signin: "your sign-in details",
  devices: "your Devices",
  playback: "your Playback settings",
  home: "your Home layout",
  requests: "your Requests",
  connections: "your Connections",
  libraries: "the Libraries",
  "needs-a-match": "what needs a Match",
  "now-playing": "what’s playing",
  jobs: "the Jobs",
  server: "the Server settings",
  users: "the Users",
  backups: "the Backups",
  "default-home": "the default Home layout",
  "server-playback": "the Server’s Playback settings",
  plugins: "the Plugins",
  account: "your Account",
  "account-signin": "your sign-in details",
  "account-sessions": "where you’re signed in",
  "account-servers": "your Servers and Users",
  "account-history": "your viewing history",
};
