#!/usr/bin/env bash
# Builds the simulator page and copies it to a web server's directory over SSH:
#
#   tools/ui-web/deploy.sh user@host:/var/www/octowhere
#
# The server needs nothing but static files. Files no longer built are removed from it.
set -euo pipefail
target=${1:?"usage: $0 user@host:/path/to/site"}
cd "$(dirname "$0")"
./build.sh
rsync -rlvz --delete --chmod=D755,F644 dist/ "$target"
