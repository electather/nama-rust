# The Hub's areas call down fixed rows, and proto types stop at `api`

The Hub has no Plugins, so the only way it grows is through its own code: a new feature is a new area, or a function on an existing one. It takes the Server's structure rules as they are (ADR 0051 and the "Structure" rules in `crates/nama-server/AGENTS.md`):

- one crate of areas, one folder each
- neighbours import only what an area's root re-exports
- each table is owned by one area
- a fixed order of rows, in which a module calls only a lower row, or a module to its right in its own row

The rows, highest first:

1. `api`
2. `recommendations`, `diagnostics`, `discovery`
3. `sync`, `viewing`
4. `links`
5. `accounts`, `titles`
6. `signing`, `mail`, `web`, `db`, `config`

Where a request came from, through trusted proxies only (ADR 0018), is the `nama-forwarded` crate, which the Server shares. `lib` and `main` start the Hub, run its subcommands and schedule its periodic work, and may call any row. Each area arrives with the first ticket that needs it.

- `links` owns every Link, whether a Server's or a User's: the device flow, a Linked Server's credential, Unlinking, and the sign-in tokens signed for Servers.
- `discovery` owns how clients reach a Linked Server: its names, its Server Addresses, reachability, the certificates' DNS records and Relays.

Unlike the Server's areas, the Hub's never use proto types. They take and return their own types, and `api` converts in both directions. The Hub API has to keep serving every Hub Plugin already deployed, and an area whose types are the wire contract can't change without changing the contract.

## Considered Options

- **Areas that use proto types, as the Server's do**: rejected for the reason above. The cost is conversion code in `api`.
- **A `servers` area owning both registration and discovery, with `links` for Users only**: rejected. A Server's Link and a User's Link share the device flow and an Account's ownership, and discovery shares neither.
- **Interfaces inside the Hub "for later", in place of Plugins**: rejected. A trait exists only where there are two implementations: a true external, such as mail, the DNS provider or the Diagnostics backend, with its production adapter and a test fake.

## Consequences

- `accounts` can't call up into the areas whose data an Account deletion removes. Instead, every table holding an Account's data references `accounts(id) ON DELETE CASCADE`, so deleting an Account is one `DELETE`. A Server learns of a User's Unlink the next time it names that User.
- The Server and the Hub run their structure checks from one shared dev-only crate, `nama-structure`. Each crate keeps its own tables in its own `tests/structure.rs`.
- Moving an area to another row is a decision recorded here, as it is for the Server.
