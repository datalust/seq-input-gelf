# `sqelf` [![Build status](https://ci.appveyor.com/api/projects/status/t32q67tvbvsgjxck?svg=true)](https://ci.appveyor.com/project/datalust/sqelf) [![Seq.Input.Gelf](https://img.shields.io/nuget/v/Seq.Input.Gelf.svg?style=flat)](https://nuget.org/packages/Seq.Input.Gelf) [![datalust/sqelf](https://img.shields.io/badge/docker-datalust%2Fsqelf-yellowgreen.svg)](https://hub.docker.com/r/datalust/sqelf)

Ingest [Graylog Extended Log Format (GELF) messages](http://docs.graylog.org/en/2.5/pages/gelf.html) via UDP into [Seq](https://datalust.co/seq). The app is packaged both as a plug-in Seq App for all platforms, and as a standalone Docker container that forwards events to Seq via its HTTP API.

## Getting started on Windows (requires Seq 5.1+)

On Windows, the GELF input is installed into Seq as a [Seq App](https://docs.getseq.net/docs/installing-seq-apps).

![Seq GELF input](https://raw.githubusercontent.com/datalust/sqelf/master/asset/app-screenshot.png)

**1. Install the app package**

In _Settings_ > _Apps_, choose _Install from NuGet_. The app package id is [Seq.Input.Gelf](https://nuget.org/packages/Seq.Input.Gelf).

**2. Start an instance of the app**

From the apps screen, choose _Add Instance_ and give the new GELF input a name.

The default settings will cause the GELF input to listen on localhost port 12201. Choose a different port if required.

Select _Save Changes_ to start the input.

**3. Configure Windows Firewall**

Ensure UDP port 12201 (or the selected port, if you specified a different one), is allowed through Windows Firewall.

**4. Log some events!**

That's all there is to it. Events ingested through the input will appear in the _Events_ stream. If the input doesn't work, check for diagnostic events raised by the input app (there is some status information shown under the app instance name).

Events ingested by the input will be associated with the default _None_ [API key](https://docs.getseq.net/docs/api-keys), which can be used to attach properties, apply filters, or set a minimum level for the ingested events.

## Getting started with Docker (all versions)

For Docker, the app is deployed as a Docker container that is expected to run alongside the Seq container. The `datalust/sqelf` container accepts UDP GELF payloads on port 12201, and forwards them to the Seq ingestion endpoint specified in the `SEQ_ADDRESS` environment variable.

To run the container:

```shell
$ docker run \
    --rm \
    -it \
    -p 12201:12201/udp \
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
| `GELF_ADDRESS` | The address to bind the UDP GELF server to. The protocol may be `udp` or `tcp` | `udp://0.0.0.0:12201` |
| `GELF_ENABLE_DIAGNOSTICS` | Whether to enable diagnostic logs and metrics (accepts `True` or `False`) | `False` |

### Quick local setup with `docker-compose`

The following is an example `docker-compose` file that can be used to manage a local Seq container alongside `sqelf` in your development environment to collect log events from other containers:

```yaml
version: '3'
services:
  sqelf:
    image: datalust/sqelf:latest
    depends_on:
      - seq
    ports:
      - "12201:12201/udp"
    environment:
      SEQ_ADDRESS: "http://seq:5341"
    restart: unless-stopped
  seq:
    image: datalust/seq:latest
    ports:
      - "5341:80"
    environment:
      ACCEPT_EULA: Y
    restart: unless-stopped
    volumes:
      - ./seq-data:/data
```

The service can be started using `docker-compose up`:

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
