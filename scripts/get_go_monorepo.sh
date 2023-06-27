#!/usr/bin/env bash

# get_go_monorepo.sh - Updates the .cached_go_monorepo.env with the full commit of the go monorepo.
#
# Syntax:   get_go_monorepo.sh
#
# Examples: get_go_monorepo.sh

set -e
set -o pipefail

GO_MONOREPO=github.com/stellar/go
SCRIPTPATH="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"

# find the short commit from the soroban-tools repository.
SHORT_COMMIT=$(cat ${SCRIPTPATH}/../go.mod | grep "${GO_MONOREPO} " | cut -d- -f3)

# check to see if we have this already cached
if [ -e "${SCRIPTPATH}/.cached_go_monorepo.env" ]; then
  # output file was already cached.
  exit 0
fi

# find the long commit from the actual go repository using the short commit.
TEMPDIR=$(mktemp -d)
git clone -q https://${GO_MONOREPO}.git ${TEMPDIR}
CURRENT_DIR=$(pwd)
cd ${TEMPDIR}
LONG_COMMIT=$(git rev-parse ${SHORT_COMMIT})
rm -rf ${TEMPDIR}
cd ${CURRENT_DIR}

echo "SHORT_COMMIT=${SHORT_COMMIT}" > ${SCRIPTPATH}/.cached_go_monorepo.env
echo "COMMIT=${LONG_COMMIT}" >> ${SCRIPTPATH}/.cached_go_monorepo.env

