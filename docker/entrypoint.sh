#!/bin/bash
# run rrdpit with variable interpolation
set -e
DATA="${DATA:-/data}"
SOURCE_DIR="${SOURCE_DIR:-$DATA/source}"
TARGET_DIR="${TARGET_DIR:-$DATA/target}"
RSYNC_URI="${RSYNC_URI:-$DATA/rsync}"
HTTPS_URI="${HTTPS_URI:-$DATA/https}"

exec /usr/local/bin/rrdpit \
    -v \
    --source ${SOURCE_DIR} \
    --target ${TARGET_DIR} \
    --rsync ${RSYNC_URI} \
    --https ${HTTPS_URI} \
    "$@"
