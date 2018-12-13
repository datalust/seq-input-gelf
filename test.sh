#! /bin/bash

set -e

# Build a local container
./ci/local-build.sh
docker image rm -f sqelf-local:latest || true
docker build --file dockerfiles/Dockerfile -t sqelf-local:latest .

# Start the Seq environment
pushd tests/seq
./up.sh
popd

# Build a test app container
docker image rm -f sqelf-app-test:latest || true
docker build --file tests/app/Dockerfile -t sqelf-app-test:latest .

# Run the test app, pointing to the GELF server
docker run \
    --rm \
    -it \
    --log-driver gelf \
    --log-opt gelf-address=udp://localhost:12201 \
    sqelf-app-test:latest

# Query the Seq API for events
curl http://localhost:5341/api/events | json_pp
