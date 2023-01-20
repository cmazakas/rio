#!/bin/bash

clear \
  && openssl genrsa -out ca.key 2048 \
  && openssl req -x509 -new -noenc -key ca.key -sha256 -days 2000 -out ca.crt \
  && openssl genrsa -out server.key 2048 \
  && openssl req -new -key server.key -out server.csr \
  && openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt -days 2000 -extfile server.v3.ext

# openssl x509 -in server.crt -noout -text
