#!/bin/bash

set -e

CURL="curl -sL --fail-with-body"
CARGO_PACKAGE_REVISION_EXTRACT_SED_COMMAND='s/.*rev=\(.*\)#.*/\1/'

if ! CARGO_OUTPUT=$(cargo tree -p soroban-env-host 2>&1); then
  echo "The project depends on multiple versions of the soroban-env-host Rust library, please unify them."
  echo "Make sure the soroban-sdk dependency indirectly points to the same soroban-env-host dependency imported explicitly."
  echo
  echo "This is soroban-env-host version imported by soroban-sdk:"
  cargo tree --depth 1  -p soroban-sdk | grep env-host
  echo
  echo
  echo
  echo "Full error:"
  echo $CARGO_OUTPUT
  exit 1
fi


# revision of the https://github.com/stellar/rs-stellar-xdr library used by the Rust code
RS_STELLAR_XDR_REVISION=""

# revision of https://github.com/stellar/stellar-xdr/ used by the Rust code
STELLAR_XDR_REVISION_FROM_RUST=""

if CARGO_OUTPUT=$(cargo tree --depth 0 -p stellar-xdr 2>&1); then
  RS_STELLAR_XDR_REVISION=$(echo $CARGO_OUTPUT | head -n 1 | sed "$CARGO_PACKAGE_REVISION_EXTRACT_SED_COMMAND")
  STELLAR_XDR_REVISION_FROM_RUST=$($CURL https://raw.githubusercontent.com/stellar/rs-stellar-xdr/${RS_STELLAR_XDR_REVISION}/xdr/next-version)
else
  echo "The project depends on multiple versions of the Rust rs-stellar-xdr library"
  echo "Make sure a single version of stellar-xdr is used"
  echo
  echo
  echo
  echo "Full error:"
  echo $CARGO_OUTPUT
fi

# Now, lets compare the Rust and Go XDR revisions
# TODO: The sed extraction below won't work for version tags
GO_XDR_REVISION=$(go list -m -f '{{.Version}}' github.com/stellar/go | sed 's/.*-\(.*\)/\1/')

# revision of https://github.com/stellar/stellar-xdr/ used by the Go code
STELLAR_XDR_REVISION_FROM_GO=$($CURL https://raw.githubusercontent.com/stellar/go/${GO_XDR_REVISION}/xdr/xdr_commit_generated.txt)

if [ "$STELLAR_XDR_REVISION_FROM_GO" != "$STELLAR_XDR_REVISION_FROM_RUST" ]; then
  echo "Go and Rust dependencies are using different revisions of https://github.com/stellar/stellar-xdr"
  echo
  echo "Rust dependencies are using commit $STELLAR_XDR_REVISION_FROM_RUST"
  echo "Go dependencies are using commit $STELLAR_XDR_REVISION_FROM_GO"
  exit 1
fi

# Now, lets make sure that the core and captive core version used in the tests use the same version and that they depend
# on the same XDR revision

# TODO: The sed extractions below won't work when the commit is not included in the Core image tag/debian packages version
CORE_CONTAINER_REVISION=$(sed -n 's/.*\/stellar-core:.*\..*-[^\.]*\.\(.*\)\..*/\1/p' < cmd/soroban-rpc/internal/test/docker-compose.yml)
CAPTIVE_CORE_PKG_REVISION=$(sed -n 's/.*DEBIAN_PKG_VERSION:.*\..*-[^\.]*\.\(.*\)\..*/\1/p' < .github/workflows/soroban-rpc.yml)

if [ "$CORE_CONTAINER_REVISION" != "$CAPTIVE_CORE_PKG_REVISION" ]; then
  echo "Soroban RPC integration tests are using different versions of the Core container and Captive Core Debian package."
  echo
  echo "Core container image commit $CORE_CONTAINER_REVISION"
  echo "Captive core debian package commit $CAPTIVE_CORE_PKG_REVISION"
  exit 1
fi

# Revision of https://github.com/stellar/rs-stellar-xdr by Core.
# We obtain it from src/rust/src/host-dep-tree-curr.txt but Alternatively/in addition we could:
#  * Check the rs-stellar-xdr revision of host-dep-tree-prev.txt
#  * Check the stellar-xdr revision
CORE_HOST_DEP_TREE_CURR=$($CURL https://raw.githubusercontent.com/stellar/stellar-core/${CORE_CONTAINER_REVISION}/src/rust/src/host-dep-tree-curr.txt)


RS_STELLAR_XDR_REVISION_FROM_CORE=$(echo "$CORE_HOST_DEP_TREE_CURR" | grep stellar-xdr | head -n 1 | sed "$CARGO_PACKAGE_REVISION_EXTRACT_SED_COMMAND")
if [ "$RS_STELLAR_XDR_REVISION" != "$RS_STELLAR_XDR_REVISION_FROM_CORE" ]; then
  echo "The Core revision used in integration tests (${CORE_CONTAINER_REVISION}) uses a different revision of https://github.com/stellar/rs-stellar-xdr"
  echo
  echo "Current repository's revision $RS_STELLAR_XDR_REVISION"
  echo "Core's revision $RS_STELLAR_XDR_REVISION_FROM_CORE"
  exit 1
fi



