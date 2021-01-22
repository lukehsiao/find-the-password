#!/bin/bash

for i in {0..10000}
do
    openssl rand -base64 32 | md5sum | awk '{print $1}'
done
