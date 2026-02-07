# Myriam

Local and remote actor model implementation with an API heavily inspired in (but not necessarily equivalent to) [Goblins](https://gitlab.com/spritely/guile-goblins).

```rust
// LocalHandle allows for local, type-safe communication
// UntypedHandle tries to do the same, but requests and responses are
// bag of bytes and have to be {de}serialized
let (local_handle, untyped_handle)
    = remote::spawn_untyped::<_, _, _, BitcodeDencoder>(Mult { a: 3 }).await?;

// create router with a TOR netlayer
let layer = TorLayer::new("myriam-foo".to_string(), TorLayerConfig::new_from_port(8081)).await?;

let router_handle = Router::with_netlayer(layer, Some(RouterOpts::default())).await?;

// routers handle external access to several attached actors
// we can think of this exposed actor as a capability
// "tor:4ruu43hmgibt5lgg3cvghbrmprotl5m7ts2lral5wnhf5wwkocva@someaddress.onion"
let address = router_handle.attach(untyped_handle).await?;

let new_layer = TorLayer::new_for_client("myriam-bar".to_string()).await?;


let remote_handle
    = RemoteHandle::<u32, u32, SomeError, BitcodeDencoder, TorNetLayer>::new(&address, new_layer);
//                     type handle once ^

// use RemoteHandle just like a LocalHandle
let res = remote_handle.send(Message::Task(42)).await?;

// capabilities can be revoked anytime
router_handle.revoke(&address).await?;

tokio::time::sleep(Duration::from_millis(100));

// ...and thus we can't invoke this one anymore
remote_handle.send(Message::Ping).await.unwrap_err();
```

Check out the examples for a more comprehensive demo.

## features

* `remote (default)`: support for remote messaging
* `tcp (default)`: TCP test-only net layer
* `tor (default)`: Tor net layer - built with [arti_client](https://gitlab.torproject.org/tpo/core/arti)
