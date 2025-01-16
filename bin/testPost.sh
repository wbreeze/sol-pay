#!/usr/bin/env bash

DATA="$(dirname $0)/testPostData.json"
ENDPOINT="http://localhost:3000/api/tx"
echo "Posting data in ${DATA} to ${ENDPOINT} using curl"
curl --json @${DATA} ${ENDPOINT}

