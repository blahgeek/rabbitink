#!/bin/bash -ex

cd "$(dirname "$0")"

docker build ./docker/
DOCKER_IMAGE_ID=$(docker build -q ./docker/)

docker run -it --rm \
       -v $(pwd)/../:/rabbitink \
       $DOCKER_IMAGE_ID \
       /rabbitink/build_arm/build_in_docker.sh
