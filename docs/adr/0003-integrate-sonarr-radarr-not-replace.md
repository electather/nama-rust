# Integrate Sonarr/Radarr, don't replace them

Requests are fulfilled by passing them to Sonarr and Radarr through their APIs. Nama does not do its own indexing, downloading or file renaming. Rebuilding the *arr stack would multiply the project's size several times over. Sonarr and Radarr are the first Installed Plugins (WASM, bundled with each release). They are built in the same milestone as the plugin runtime, so the plugin contract is tested against a real use from the start.
