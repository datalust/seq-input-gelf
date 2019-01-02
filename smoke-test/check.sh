# Start the Seq environment
docker-compose -p sqelf-test rm -f
rm -rf seq-data || true

docker-compose -p sqelf-test build
docker-compose -p sqelf-test up -d

# Build a test app container
docker image rm -f sqelf-app-test:latest || true
docker build --file smoke-test/app/Dockerfile -t sqelf-app-test:latest .

# Run the test app, pointing to the GELF server
docker run \
    --rm \
    -it \
    --log-driver gelf \
    --log-opt gelf-address=udp://localhost:12201 \
    sqelf-app-test:latest

sleep 2s

# Query the Seq API for events
curl http://localhost:5341/api/events?clef

pushd dockerfiles
docker-compose -p sqelf-test down
docker-compose -p sqelf-test rm -f
popd