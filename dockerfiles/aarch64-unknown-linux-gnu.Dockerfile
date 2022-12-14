FROM --platform=linux/arm64 datalust/seqcli:latest

COPY target/aarch64-unknown-linux-gnu/release/sqelf /bin/sqelf
COPY dockerfiles/run.sh /run.sh

EXPOSE 12201

ENV SEQ_ADDRESS=http://localhost:5341
ENV SEQ_API_KEY=
ENV GELF_ADDRESS=

ENTRYPOINT ["/run.sh"]
