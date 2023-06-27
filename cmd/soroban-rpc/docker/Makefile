SUDO := $(shell docker version >/dev/null 2>&1 || echo "sudo")

# https://github.com/opencontainers/image-spec/blob/master/annotations.md
BUILD_DATE := $(shell date -u +%FT%TZ)

# Extract latest release semver from GitHub
SOROBAN_RPC_LATEST_RELEASE := $(shell curl -sS https://api.github.com/repos/stellar/soroban-tools/releases/latest|jq -r ".tag_name"| tr -d "v" )

# If deb version was provided via the SOROBAN_RPC_VERSION variable use it.
# If not get latest deb build matching release from GitHub
ifndef SOROBAN_RPC_VERSION
        SOROBAN_RPC_VERSION_PACKAGE_VERSION := $(shell curl -sS https://apt.stellar.org/dists/focal/unstable/binary-amd64/Packages|grep -A 18 stellar-soroban-rpc|grep Version|grep $(SOROBAN_RPC_LATEST_RELEASE)|head -1|cut -d' ' -f2 )
else
        SOROBAN_RPC_VERSION_PACKAGE_VERSION := $(SOROBAN_RPC_VERSION)
endif

ifndef SOROBAN_RPC_VERSION_PACKAGE_VERSION
        $(error Couldn't establish deb build from version $(SOROBAN_RPC_LATEST_RELEASE). Has the package been built?)
endif

ifndef STELLAR_CORE_VERSION
        $(error STELLAR_CORE_VERSION environment variable must be set. For example 19.10.1-1310.6649f5173.focal~soroban)
endif

TAG ?= stellar/stellar-soroban-rpc:$(SOROBAN_RPC_VERSION_PACKAGE_VERSION)

docker-build:
	$(SUDO) docker build --pull --platform linux/amd64 $(DOCKER_OPTS) \
	--label org.opencontainers.image.created="$(BUILD_DATE)" \
	--build-arg STELLAR_CORE_VERSION=$(STELLAR_CORE_VERSION) --build-arg SOROBAN_RPC_VERSION=$(SOROBAN_RPC_VERSION_PACKAGE_VERSION) \
	-t $(TAG) -f Dockerfile.release .

docker-push:
	$(SUDO) docker push $(TAG)
