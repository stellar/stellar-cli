SUDO := $(shell docker version >/dev/null 2>&1 || echo "sudo")

# https://github.com/opencontainers/image-spec/blob/master/annotations.md
BUILD_DATE := $(shell date -u +%FT%TZ)

TAG ?= stellar/stellar-soroban-rpc:$(SOROBAN_RPC_VERSION)

STELLAR_CORE_VERSION ?= 19.10.1-1310.6649f5173.focal~soroban
SOROBAN_RPC_VERSION ?= 0.8.0-21

docker-build:
	$(SUDO) docker build --pull $(DOCKER_OPTS) \
	--label org.opencontainers.image.created="$(BUILD_DATE)" \
	--build-arg STELLAR_CORE_VERSION=$(STELLAR_CORE_VERSION) --build-arg SOROBAN_RPC_VERSION=$(SOROBAN_RPC_VERSION) \
	-t $(TAG) -f Dockerfile.release .

docker-push:
	$(SUDO) docker push $(TAG)
