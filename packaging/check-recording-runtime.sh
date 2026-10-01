#!/usr/bin/env bash
set -euo pipefail
# Never requests a screen, microphone, or camera. Runs WebKit on generated media.
repo_dir="$(cd "$(dirname "$0")/.." && pwd)"
app_dir="$(realpath "${1:?Provide the built Continuum.AppDir directory}")"
shift
plugin_dir="$app_dir/usr/lib/Continuum/gstreamer-1.0"
[[ -x "$plugin_dir/gst-plugin-scanner" ]] || { echo "Missing packaged scanner" >&2; exit 1; }
probe_dir="$(mktemp -d /tmp/continuum-runtime-check-XXXXXX)"
export LD_LIBRARY_PATH="$app_dir/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export GST_PLUGIN_PATH_1_0="$plugin_dir"
export GST_PLUGIN_SYSTEM_PATH_1_0=""
export GST_PLUGIN_SCANNER_1_0="$plugin_dir/gst-plugin-scanner"
export GST_REGISTRY="$probe_dir/registry.bin"
export GDK_BACKEND=x11
# AppImage relocates WebKit's helper paths relative to this directory.
cd "$app_dir/usr"
exec /usr/bin/python3 "$repo_dir/apps/continuum-desktop/qa/recording_probe.py" "$@"
