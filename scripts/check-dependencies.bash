#!/bin/bash
set -e

if ! cargo tree -p soroban-env-host &> /dev/null; then
  echo "The project depends multiple versions of the soroban-env-host Rust library, please unify them."
  echo "Make sure the soroban-sdk dependency indirectly points to the same soroban-env-host dependency imported explicitly."
  echo
  echo "This is soroban-env-host version imported by soroban-sdk:"
  cargo tree --depth 1  -p soroban-sdk | grep env-host
  exit 1
fi


STELLAR_XDR_REVISION_FROM_RUST=""

if RUST_STELLAR_XDR_REVISION=$(cargo tree --depth 1 -p stellar-xdr 2> /dev/null | head -n 1 | sed 's/.*rev=\(.*\)#.*/\1/'); then
  STELLAR_XDR_REVISION_FROM_RUST=$(curl -sL https://raw.githubusercontent.com/stellar/rs-stellar-xdr/${RUST_STELLAR_XDR_REVISION}/xdr/next-version)
else
  echo "The project depends on multiple versions of the Rust rs-stellar-xdr library"
  echo "Make sure a single version of stellar-xdr is used"
  exit 1
fi

# Now, lets compare the Rust and Go XDR revisions
GO_XDR_REVISION=$(go list -m all | grep 'github.com/stellar/go ' |  sed 's/.*-\(.*\)/\1/')
STELLAR_XDR_REVISION_FROM_GO=$(curl -sL https://raw.githubusercontent.com/stellar/go/${GO_XDR_REVISION}/xdr/xdr_commit_generated.txt)

if [ "$STELLAR_XDR_REVISION_FROM_GO" != "$STELLAR_XDR_REVISION_FROM_RUST" ]; then
  echo "Go and Rust dependencies are using different revisions of https://github.com/stellar/stellar-xdr"
  echo "Rust dependencies are using commit $STELLAR_XDR_NEXT_REVISION_FROM_RUST"
  echo "Go dependencies are using commit $STELLAR_XDR_REVISION_FROM_GO"
  exit 1
fi

# Now, lets make sure that the core and captive core version used in the tests use the same version and that they depend
# on the same XDR revision

# TODO: The sed extractions below won't work when the commit is not included in the Core image/debian packages

CORE_CONTAINER_REVISION=$(sed -n 's/.*\/stellar-core:.*\..*-[^\.]*\.\(.*\)\..*/\1/p' < cmd/soroban-rpc/internal/test/docker-compose.yml)
CAPTIVE_CORE_PKG_REVISION=$(sed -n 's/.*DEBIAN_PKG_VERSION:.*\..*-[^\.]*\.\(.*\)\..*/\1/p' < .github/workflows/soroban-rpc.yml)

echo $CORE_CONTAINER_REVISION

if [ "$CORE_CONTAINER_REVISION" != "$CAPTIVE_CORE_PKG_REVISION" ]; then
  echo "Soroban RPC integration tests are using different versions of the Core container and Captive Core Debian package"
  echo "Core container image commit $CORE_CONTAINER_REVISION"
  echo "Captive core debian package commit $STELLAR_XDR_REVISION_FROM_GO"
  exit 1
fi

# TODO: extract the XDR version used by core and compare it with the XDR version of soroban-tools
#       (Probably based on the contents src/rust/src/host-dep-tree-curr.txt)






