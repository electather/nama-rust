# The Kernel deletes media, and Fulfillers only stop monitoring it

A User with `DELETE_MEDIA` can delete a Movie, a Show, a Season, an Episode or one Version. The Kernel deletes the files itself, using the Library's own paths, after asking every enabled Fulfiller of that kind to stop monitoring what's going, by its tmdb or tvdb id. It asks first, so Sonarr or Radarr can't notice the missing file and fetch it again before it hears. Only the Kernel knows the Server's paths. Sonarr's and Radarr's paths can differ, and nothing maps one to the other. The Kernel also can't tell which Fulfiller manages a title nobody Requested, so it asks them all, and one that doesn't have the title does nothing. A future cleanup Plugin (#700) uses the same Kernel operation.

## Considered Options

- **Sonarr or Radarr deletes the files** (`deleteFiles`, then a rescan): rejected. It needs a mapping from the *arr's paths to the Server's. It also needs a second way to delete titles no Fulfiller manages, and the delete only shows once a scan notices it.
- **A Fulfiller Intercepts the delete and takes it over**: rejected. That's a lot of machinery for one use, and it would still need the Kernel's way for every other title.

## Consequences

- A Fulfiller that can't be reached doesn't block the delete. The person deleting is told which Fulfillers weren't told and might download the title again. The Kernel doesn't try them again later.
- Stopping monitoring never removes a title from Sonarr or Radarr, and it never stops new Seasons from arriving. A whole Show's delete stops monitoring its current Seasons and Episodes, but the series stays monitored for new ones, and its Followed Show stays. That way watched Seasons can be cleaned up while new ones still come.
- Deleting one Version of several still stops monitoring the Movie or Episode, or the *arr would download the missing file again.
- A file that holds several Episodes goes with all of them.
- Deleting is permanent: nothing goes to a trash folder.
- A Library folder must be writable for its media to be deleted. The README's `:ro` mount means the delete is refused.
