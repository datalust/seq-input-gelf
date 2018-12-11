#!/bin/bash

exec bin/sqelf | bin/seqcli/seqcli ingest --json -a $SEQ_ADDRESS
