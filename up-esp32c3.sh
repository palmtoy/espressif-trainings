#!/bin/bash

docker run --name http-server_esp32c3 --rm \
	-v $HOME/Repository/Software/Programming/IDF-Rust/registry:/home/esp/.cargo/registry \
	-v $HOME/Workspace/GitHub/espressif-trainings:/espressif-trainings \
	-w /espressif-trainings/intro/http-server/solution \
	-it espressif/idf-rust:esp32c3_v4.4_1.62.0.0_classic

