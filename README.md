# ParallelChain Network

Implementation of [ParallelChain protocol](https://github.com/parallelchain-io/parallelchain-protocol) peer-to-peer (P2P) networking.

ParallelChain Network is a combination of **4** [libp2p](https://crates.io/crates/libp2p) protocols running on top of a [Noise](https://docs.libp2p.io/concepts/secure-comm/noise/)-authenticated TCP transport:
1. [Kademlia](https://github.com/libp2p/specs/tree/master/kad-dht) forms and maintains a connected and efficient network topology: every peer can reach every other peer in a small number of hops.
2. [Identify](https://github.com/libp2p/specs/tree/master/identify) lets peers inform other peers of changes in their basic information.
3. [Ping](https://github.com/libp2p/specs/blob/master/ping/ping.md) lets peers quickly check the liveness of other peers.
4. [Gossipsub](https://github.com/libp2p/specs/tree/master/pubsub/gossipsub) implements the primary useful function of ParallelChain Network: general publish/subscribe messaging.

## Usage

Starting a `pchain_network` peer: [docs.rs](https://docs.rs/pchain_network/0.4.2/pchain_network/#starting-a-peer).

## Versioning

The version of this library reflects the version of the ParallelChain Protocol which it implements. For example, the current version is 0.4.2, and this implements protocol version 0.4. Patch version increases are not guaranteed to be non-breaking.

## Opening an issue

Open an issue in GitHub if you:
1. Have a feature request / feature idea,
2. Have any questions (particularly software related questions),
3. Think you may have discovered a bug.

Please try to label your issues appropriately.