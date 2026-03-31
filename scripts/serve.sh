#!/usr/bin/env bash
set -euo pipefail

PORT=9507

exec trunk serve --port "$PORT" "$@"
