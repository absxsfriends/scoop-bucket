#!/bin/bash
set -euxo pipefail

wget https://github.com/absxsfriends/scoop-bucket/releases/download/v0.1.0/auto-update-x86_64-unknown-linux-musl.tar.gz -o /tmp/au.tar.gz
tar -xzf /tmp/au.tar.gz -C /tmp
mv /tmp/auto-update /usr/local/bin/
chmod +x /usr/local/bin/auto-update
auto-update bucket