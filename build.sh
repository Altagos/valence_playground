#!/usr/bin/env bash

sudo systemctl stop valence_playground.service
cargo build --release
cp target/release/valence_playground bin/valence_playground
sudo systemctl start valence_playground.service