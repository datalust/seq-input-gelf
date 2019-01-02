# `sqelf`

A server that accepts [Graylog extended format messages](http://docs.graylog.org/en/2.5/pages/gelf.html) via UDP and writes them to [Seq](https://datalust.co/seq).

This repository contains an example `docker-compose` file that can be used to manage a local Seq container alongside `sqelf` in your development environment to collect log events from other containers:

```shell
$ docker-compose -p seq up -d
```

The server is also available as a standalone container on Docker Hub as [`datalust/sqelf`](https://hub.docker.com/r/datalust/sqelf).

The output from any Docker container can then be collected by configuring its logging driver on startup:

```shell
$ docker run \
    --rm \
    -it \
    --log-driver gelf \
    --log-opt gelf-address=udp://localhost:12201 \
    my-app:latest
```

## Configuration

A `sqelf` container can be configured using the following environment variables:

| Variable | Description | Default |
| -------- | ----------- | ------- |
| `SEQ_ADDRESS`| The address of the Seq server to forward events to | `http://localhost:5341` |
| `SEQ_API_KEY` | The API key to use | - |
