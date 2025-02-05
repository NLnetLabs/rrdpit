#!/usr/bin/env bash

set -eo pipefail
set -x

case $1 in
  post-install)
    echo -e "\nRRDPIT VERSION:"
    rrdpit --version
    ;;

  post-upgrade)
    ;;
esac