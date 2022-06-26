# Changelog

## Version 0.6.0

Almost complete rewrite of `myriam` on top of `libp2p`.

- Actors now have their own libp2p swarm handling their connections, requests and replies.
- Removed custom address and keypair objects in favor of those already offered by libp2p.
