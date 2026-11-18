## web/src/routes/\_member/\_admin

- **Admin pages need a Permission.** Every page here has a row in `adminPages` (`src/lib/access.ts`) naming the Permission it needs. The `_admin` layout refuses a page without one to everybody, and the row also puts it in the Admin nav.
