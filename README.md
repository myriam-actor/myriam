# myriam-rs

Remote actors with e2e encryption.

Displaimer: This API is a. EXTREMELY unstable at the moment and b. not guaranteed to be secure. Please don't even think of letting this get close to a production system, or anything you even remotely care about for that matter.

# Usage

## Configuration

* Message size cap (in bytes): can be set with via env var `MYRIAM_MAX_MSG_SIZE`. Default is `8_388_608`.
* Message recv timeout (in milliseconds): can be set via `ActorOpts` when spawning or globally with the env var `MYRIAM_READ_TIMEOUT`.

See the `example` directory for the nitty-gritty.

# Caveats/TODOs

* This is all far from being ergonomic, despite simplicity and ease of use being my main focus. If you have any suggestions for improvements, please open an issue!

# License

(c) Ariela Wenner - 2022

Licensed under the [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0.txt).