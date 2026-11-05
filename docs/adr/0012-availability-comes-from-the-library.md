# Request availability comes from the Library, not from Sonarr/Radarr

The Kernel owns every Request and its states (`Pending → Approved | Declined → Processing → Partially Available → Available`, plus `Failed`). The fulfiller Provider only hands the work off. A Request moves to Partially Available or Available only when the Server's own Library scan Matches the requested Movie or Episodes. Seerr has to ask Sonarr/Radarr and the media server to find this out; because nama has requests built in, the Library is the single source of truth and there's nothing to poll.
