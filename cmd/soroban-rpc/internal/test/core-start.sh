#!/usr/bin/env bash

set -e
set -x

source /etc/profile
# work within the current docker working dir
if [ ! -f "./stellar-core.cfg" ]; then
   cp /stellar-core.cfg ./
fi   

echo "using config:"
cat stellar-core.cfg

# initialize new db
stellar-core new-db

if [ "$1" = "standalone" ]; then
  # initialize for new history archive path, remove any pre-existing on same path from base image
  rm -rf ./history
  stellar-core new-hist vs

  # serve history archives to horizon on port 1570
  pushd ./history/vs/
  python3 -m http.server 1570 &
  popd
fi

exec stellar-core run --console
