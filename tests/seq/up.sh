#! /bin/bash

set -e

docker network create seq-net || true
docker-compose rm -f
docker-compose build
docker-compose up -d
