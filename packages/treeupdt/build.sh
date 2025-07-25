#!/usr/bin/env bash
set -euo pipefail

echo "Building treeupdt..."
go build -o treeupdt ./cmd/treeupdt

echo "Running tests..."
go test ./...

echo "Build successful!"
