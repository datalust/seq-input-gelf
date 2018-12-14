# `sqelf`

A server that accepts [Graylog extended format messages](http://docs.graylog.org/en/2.5/pages/gelf.html) via UDP and writes them as [compact log events](https://github.com/serilog/serilog-formatting-compact) to stdout.

This repository contains an example `docker-compose` file that can be used to manage a local Seq container alongside Sqelf to collect log events from other containers:

```shell
$ docker-compose -p seq up -d
```

The output from any Docker container can then be collected by configuring its logging driver on startup:

```shell
$ docker run \
    --rm \
    -it \
    --log-driver gelf \
    --log-opt gelf-address=udp://localhost:12201 \
    my-app:latest
```
