# The Server with jellyfin-ffmpeg, a known-good ffmpeg with VAAPI, QSV, NVENC and tone-mapping
# (ADR 0026). `.github/workflows/docker.yml` builds it from the release's Linux binaries, which it
# puts at `<arch>/nama-server` beside this file.
FROM debian:trixie-slim

LABEL org.opencontainers.image.source=https://github.com/electather/nama-rust

# Pinned so a swapped key on Jellyfin's host can't sign what gets installed. It has no expiry.
ADD --chmod=644 --checksum=sha256:a0cde241ae297fa6f0265c0bf15ce9eb9ee97c008904a59ab367a67d59532839 \
    https://repo.jellyfin.org/jellyfin_team.gpg.key /usr/share/keyrings/jellyfin.asc
# jellyfin-ffmpeg bundles the Intel and AMD VA-API drivers, so VAAPI and QSV need only /dev/dri.
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && printf 'Types: deb\nURIs: https://repo.jellyfin.org/debian\nSuites: trixie\nComponents: main\nSigned-By: /usr/share/keyrings/jellyfin.asc\n' \
        > /etc/apt/sources.list.d/jellyfin.sources \
    && apt-get update \
    && apt-get install --yes --no-install-recommends jellyfin-ffmpeg8 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --uid 1000 --user-group --home-dir /data --shell /usr/sbin/nologin nama \
    && install --directory --owner=nama --group=nama /data

ARG TARGETARCH
COPY --chmod=755 ${TARGETARCH}/nama-server /usr/local/bin/nama-server

# The NVIDIA container runtime mounts NVENC's libraries only when `video` is asked for. The mmap
# threshold hands a finished image resize's memory back to the system, which glibc would otherwise
# keep (ADR 0039).
ENV NVIDIA_DRIVER_CAPABILITIES=compute,video,utility \
    GLIBC_TUNABLES=glibc.malloc.mmap_threshold=131072 \
    NAMA_DATA_DIR=/data \
    NAMA_FFMPEG=/usr/lib/jellyfin-ffmpeg/ffmpeg \
    NAMA_FFPROBE=/usr/lib/jellyfin-ffmpeg/ffprobe

USER 1000:1000
# The working directory, so a `nama.toml` in /data is read.
WORKDIR /data
VOLUME /data
EXPOSE 7980
ENTRYPOINT ["nama-server"]
