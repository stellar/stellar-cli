#!/usr/bin/env bash

# get_core_docker_image.sh - update the associated core docker image.
#
# Syntax:   get_core_docker_image.sh
#
# Examples: get_core_docker_image.sh

set -e
set -o pipefail

SCRIPTPATH="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"

# check to see if we have this already cached
if [ -e "${SCRIPTPATH}/.cached_core_docker_image.env" ]; then
  # output file was already cached.
  exit 0
fi

if [ ! -e "${SCRIPTPATH}/.cached_go_monorepo.env" ]; then
  # run the get_go_monorepo.sh
  if ! ${SCRIPTPATH}/get_go_monorepo.sh; then
    exit 1
  fi
fi

source ${SCRIPTPATH}/.cached_go_monorepo.env
COMMIT=$(grep -E "^COMMIT=.*" <${SCRIPTPATH}/.cached_go_monorepo.env | sed -E 's/.*COMMIT=(.*)/\1/')
HORIZON_TEST_FILE=https://raw.githubusercontent.com/stellar/go/${COMMIT}/.github/workflows/horizon.yml
TEMP_HORIZON_YML_FILE=$(mktemp -q)

if ! curl -s ${HORIZON_TEST_FILE} -o ${TEMP_HORIZON_YML_FILE}; then
  echo "unable to retrieve ${HORIZON_TEST_FILE}"
  rm -f ${TEMP_HORIZON_YML_FILE}
  exit 1
fi

CORE_DOCKER_IMAGE=$(cat ${TEMP_HORIZON_YML_FILE} | grep -E ".*PROTOCOL_20_CORE_DOCKER_IMG:.*" | sed -E 's/.*PROTOCOL_20_CORE_DOCKER_IMG:(.*)/\1/' | sed 's/ //g')
rm -f ${TEMP_HORIZON_YML_FILE}
echo "CORE_DOCKER_IMAGE=${CORE_DOCKER_IMAGE}" > ${SCRIPTPATH}/.cached_core_docker_image.env

