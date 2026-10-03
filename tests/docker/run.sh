#!/bin/sh
# Provision online, then run sequentially as non-root without network access.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
image=${RFIG_TEST_IMAGE:-rust:1.91-bookworm}
name="rfig-verify-$$"
results=${RFIG_RESULTS_DIR:-"$root/target/docker-results/$(date +%Y%m%d-%H%M%S)-$$"}
mkdir -p "$results"
cleanup() {
  code=$?
  trap - EXIT INT TERM
  docker cp "$name:/work/results/." "$results/" 2>/dev/null || :
  docker cp "$name:/work/bash44-source.sha256" "$results/" 2>/dev/null || :
  docker inspect "$name" --format '{{.Image}} init={{.HostConfig.Init}} cpus={{.HostConfig.NanoCpus}} memory={{.HostConfig.Memory}} pids={{.HostConfig.PidsLimit}}' > "$results/container.txt" 2>/dev/null || :
  docker rm -f "$name" >/dev/null 2>&1 || :
  printf 'Results: %s\n' "$results"
  exit "$code"
}
trap cleanup EXIT INT TERM
docker run -d --init --name "$name" --cpus=2 --memory=2g --pids-limit=256 \
  -v "$root:/source:ro" "$image" sleep infinity >/dev/null
docker exec -e BUILD_BASH44="${BUILD_BASH44:-1}" "$name" sh /source/tests/docker/bootstrap.sh
docker network disconnect bridge "$name"
docker exec --user tester -w /work "$name" python3 tests/docker/run_matrix.py
