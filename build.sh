#! /bin/bash

set -e

./ci/native/local-build.sh

docker build --file dockerfiles/Dockerfile -t sqelf-local:latest .
