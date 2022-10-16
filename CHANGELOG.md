# Changelog

## Version 0.0.7

Dependency upgrade. Most notably, switch to latest `libp2p` version.

We now use specific tokio features from `libp2p`.

## Version 0.0.6

Almost complete rewrite of `myriam` on top of `libp2p`.

- Actors now have their own libp2p swarm handling their connections, requests and replies.
- Removed custom address and keypair objects in favor of those already offered by libp2p.
