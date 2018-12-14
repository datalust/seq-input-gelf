#!/bin/bash

set -e

# Build a local container
./ci/local-build.sh
docker image rm -f datalust/sqelf:latest || true
docker build --file dockerfiles/Dockerfile -t datalust/sqelf:latest .
