#!/usr/bin/env bash

TAR="$1"
# VER is either git sha or semantic version
VER="$2"

DOCKER_REGISTRY=${DOCKER_REGISTRY:-"ghcr.io/eqtylab"}

RES=$(docker load -i $TAR)

# Full image name
IMAGE_FULL=${RES#*Loaded image: }
echo "Loaded image: $IMAGE_FULL"

# Image name without dev tag
IMAGE=${IMAGE_FULL%%:*}
echo "Image name: $IMAGE"

if [ -z "$VER" ]; then
    echo "Error: VERSION not supplied."
    exit 1
else
    echo "Tagging $IMAGE_FULL as $DOCKER_REGISTRY/$IMAGE:$VER"
    docker tag "$IMAGE_FULL" "$DOCKER_REGISTRY/$IMAGE:$VER"
    docker push $DOCKER_REGISTRY/$IMAGE:$VER
fi
