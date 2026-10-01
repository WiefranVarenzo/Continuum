#!/usr/bin/env bash
set -euo pipefail

output_dir="$(cd "$(dirname "$0")" && pwd)/gstreamer-1.0"
mkdir -p "$output_dir"

scanner=""
for candidate in /usr/libexec/gstreamer-1.0/gst-plugin-scanner /usr/lib/x86_64-linux-gnu/gstreamer1.0/gstreamer-1.0/gst-plugin-scanner; do
  if [[ -x "$candidate" ]]; then
    scanner="$candidate"
    break
  fi
done

if [[ -z "$scanner" ]]; then
  echo "GStreamer plugin scanner was not found. Install the GStreamer runtime before packaging." >&2
  exit 1
fi

install -m 0755 "$scanner" "$output_dir/gst-plugin-scanner"

plugin_dir=""
for candidate in /usr/lib64/gstreamer-1.0 /usr/lib/x86_64-linux-gnu/gstreamer-1.0; do
  if [[ -f "$candidate/libgstapp.so" ]]; then
    plugin_dir="$candidate"
    break
  fi
done

if [[ -z "$plugin_dir" ]]; then
  echo "GStreamer plugins were not found. Install the base, good, and PipeWire GStreamer plugins before packaging." >&2
  exit 1
fi

plugins=(
  # WebKit MediaRecorder constructs uritranscodebin, which in turn needs
  # encodebin and URI playback/typefinding. AppImage relocates GStreamer's
  # system search path: relying on the host plugins silently breaks recording.
  libgstcoreelements.so
  libgsttypefindfunctions.so
  libgstplayback.so
  libgstencoding.so
  libgsttranscode.so
  libgstvideorate.so
  libgstaudiorate.so
  libgstautoconvert.so
  libgstvideofilter.so
  libgstvolume.so
  libgstaudiofx.so
  libgstdeinterlace.so
  libgstdebugutilsbad.so
  libgstopusparse.so
  libgstapp.so
  libgstpipewire.so
  libgstpulseaudio.so
  libgstautodetect.so
  libgstvideoconvertscale.so
  # PipeWire screen streams may use DMA-BUF frames. CPU canvas tests do not
  # exercise the GL upload/download path required by WebKit for these frames.
  libgstopengl.so
  libgstaudioconvert.so
  libgstaudiomixer.so
  libgstaudioresample.so
  libgstrawparse.so
  libgstmatroska.so
  libgstvpx.so
  libgstopus.so
)

for plugin in "${plugins[@]}"; do
  if [[ ! -f "$plugin_dir/$plugin" ]]; then
    echo "Required GStreamer plugin is missing: $plugin_dir/$plugin" >&2
    exit 1
  fi
  install -m 0644 "$plugin_dir/$plugin" "$output_dir/$plugin"
done
