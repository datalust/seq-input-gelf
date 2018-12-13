#!/bin/bash

exec bin/sqelf | bin/seqcli/seqcli ingest --json -s $SEQ_ADDRESS
