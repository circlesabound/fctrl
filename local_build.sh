#!/bin/sh

if [ -z "$(git status --porcelain)" ]; then
    GIT_COMMIT_HASH="$(git rev-parse HEAD)"
else
    GIT_COMMIT_HASH="$(git rev-parse HEAD)-dirty"
fi

env DOCKER_BUILDKIT=1 docker compose -f docker-compose.yml -f docker-compose.local.yml build --build-arg GIT_COMMIT_HASH=$GIT_COMMIT_HASH
