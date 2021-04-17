#!/bin/sh

DOCKER_BUILDKIT=1 docker-compose -f docker-compose.yml -f docker-compose.local.yml build