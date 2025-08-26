.PHONY: default
default: all

BUILD := _build
BIN := ${BUILD}/bin
IMAGES := ${BUILD}/images

KERNEL := $(shell uname -s | tr '[:upper:]' '[:lower:]')

services = \
cid-router \
azure-blob-storage-crp \
github-crp

bin-targets := $(addprefix bin., ${services})
image-targets := $(addprefix image., ${services})
image-push-targets := $(addsuffix .push, ${image-targets})

.PHONY: all bin.all image.all image.all.push
all: bin.all image.all
bin.all: ${bin-targets}
image.all: ${image-targets}
image.all.push: ${image-push-targets}

${bin-targets}: bin.%: | ${BIN}
	nix build .#$* -L -o tmp/result
	cp tmp/result/bin/* ${BIN}/
	chmod 755 ${BIN}/*

${image-targets}: image.%: | ${IMAGES}
ifeq ($(KERNEL), darwin)
	docker run --rm \
		--volume $(shell pwd):/w \
		--workdir /w \
		--mount type=volume,source=docker_nix_cache,target=/nix \
		nixpkgs/nix-flakes:nixos-23.05 \
	sh -c "nix build .#$*-image -L --option sandbox false && cp -rL result  ${IMAGES}/$*.tar.gz && rm result"
else
	nix build .#$*-image -L -o ${IMAGES}/$*.tar.gz
endif
	docker load < ${IMAGES}/$*.tar.gz

DEFAULT_IMAGE_VERSION := $(shell git rev-parse --short=7 HEAD)
${image-push-targets}: image.%.push:
	export VER=$${VERSION:-${DEFAULT_IMAGE_VERSION}} && \
	echo "Pushing image $* with version $$VER" && \
	./scripts/tag-and-push-docker-image.sh ${IMAGES}/$*.tar.gz $$VER

${BUILD} ${BIN} ${IMAGES}:
	mkdir -p $@

.PHONY: clean
clean:
	rm -rf ${BUILD}
