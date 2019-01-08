# `sqelf`

An app that accepts [Graylog Extended Log Format (GELF) messages](http://docs.graylog.org/en/2.5/pages/gelf.html) via UDP and writes them to [Seq](https://datalust.co/seq).

## Getting started on Windows

On Windows, the GELF input is installed into Seq as a Seq App.

> **Note:** packaging for Seq on Windows is still in progress; please track this via [#10](https://github.com/datalust/sqelf/issues/10).

## Getting started with Docker

For Docker, the app is deployed as a Docker container that is expected to run alongside the Seq container. The `datalust/sqelf` container accepts UDP GELF payloads on port 12201, and forwards them to the Seq ingestion endpoint specified in the `SEQ_ADDRESS` environment variable.

To run the container:

```shell
$ docker run \
    --rm \
    -it \
    -p 12201:12201 \
    -e SEQ_ADDRESS=https://seq.example.com \
    datalust/sqelf
```

The container is published on Docker Hub as [`datalust/sqelf`](https://hub.docker.com/r/datalust/sqelf).

### Container configuration

A `sqelf` container can be configured using the following environment variables:

| Variable | Description | Default |
| -------- | ----------- | ------- |
| `SEQ_ADDRESS`| The address of the Seq server to forward events to | `http://localhost:5341` |
| `SEQ_API_KEY` | The API key to use | - |

### Quick local setup with `docker-compose`

This repository contains an example `docker-compose` file that can be used to manage a local Seq container alongside `sqelf` in your development environment to collect log events from other containers:

```shell
$ docker-compose -p seq up -d
```

### Collecting Docker container logs

The output from any Docker container can be collected by configuring its logging driver on startup:

```shell
$ docker run \
    --rm \
    -it \
    --log-driver gelf \
    --log-opt gelf-address=udp://sqelf.example.com:12201 \
    my-app:latest
```

In this case the `gelf-address` option needs to resolve to the running `sqelf` container.
