#!/bin/bash
# run rrdpit with variable interpolation
set -e
DATA="${DATA:-/data}"
SOURCE_DIR="${SOURCE_DIR:-$DATA/source}"
TARGET_DIR="${TARGET_DIR:-$DATA/target}"
RSYNC_URI="${RSYNC_URI:-rsync://example.org/test/}"
HTTPS_URI="${HTTPS_URI:-https://example.org/}"

exec /usr/local/bin/rrdpit \
    --source ${SOURCE_DIR} \
    --target ${TARGET_DIR} \
    --rsync ${RSYNC_URI} \
    --https ${HTTPS_URI} \
    "$@"
