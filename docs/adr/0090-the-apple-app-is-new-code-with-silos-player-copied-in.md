# The Apple app is new code with Silo's player copied in

The iOS and tvOS app (ADR 0089) lives in `apple/` in this repo, so a proto change and its Swift code land in one commit. It is a new SwiftUI app, designed for iOS 27 and also running on iOS and tvOS 26. Its API client is generated from `proto/` with connect-swift, as ADR 0007 promised. Playback is the exception: the app uses AetherEngine through Silo's fork and copies in [Silo Apple](https://github.com/Silo-Server/silo-apple)'s player and download modules. That's where Silo's fixes for subtitles, HLS authorization, the tvOS remote, picture-in-picture and AirPlay live. A NOTICE in `apple/` names the Silo commit they came from. Nothing else is taken, and no Silo name, logo or bundle ID is used (Silo's `TRADEMARK.md`).

Silo Apple is AGPL-3.0-or-later, like Nama, with two additional permissions under AGPL §7: one for App Store distribution and its DRM and re-signing, and one for linking Apple's SDKs. Those permissions cover only Silo's copyright. Without the same grant, Nama's own code in the app couldn't be shipped to the App Store by anyone but its copyright holders; VLC was pulled from the App Store in 2011 over this. So `apple/` carries both permissions, granted for Nama's code too, and CONTRIBUTING says a contribution to `apple/` grants them. The Server stays plain AGPL.

## Considered Options

- **Fork Silo Apple whole and replace its API**: rejected. Its 1,142 Swift files hold a hand-written REST client, Silo's own domain words and screens that the iOS 27 redesign replaces anyway, so nearly every file would change. Agents working in a fork would keep tripping over Silo's model instead of this repo's GLOSSARY and ADRs.
- **Write the player too**: rejected. It is the hardest part to get right on real devices and the part Silo has already hardened.
- **A separate `nama-apple` repo**: rejected. Every proto change would need a second PR and a version to keep in sync.
- **Hand-written Swift models, as Silo has**: rejected. They drift from the protos, which the generated TypeScript and Rust code can't.

## Consequences

- `proto/buf.gen.yaml` gains a Swift target, and `apple/` gets a macOS CI job in `ci-ok`'s `needs`.
- Removing the App Store permission from `apple/` would stop Silo's copied code from shipping to the App Store, so it stays as long as that code does.
- A plain-HTTP LAN Server is reached through `NSAllowsLocalNetworking` only. A Server outside the LAN needs HTTPS, from a reverse proxy now or the Hub's per-Server certificates later (ADR 0057).
