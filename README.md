# myriam-rs

This is a WIP rewrite from scratch. I'm aiming for an API heavily inspired in [Goblins](https://gitlab.com/spritely/guile-goblins).

```rust
{
    // LocalHandle allows for local, type-safe communication
    // UntypedHandle tries to do the same, but requests and responses are
    // bag of bytes and have to be {de}serialized
    let (local_handle, untyped_handle) = Box::new(MyActor::default()).spawn();
    
    // create router with a TOR netlayer
    let mut router_handle = Router::with_netlayer(OnionNetLayer::default());

    // register additional netlayers if needed
    router_handle.register_netlayer(CustomNetLayer::new());
    
    // routers handle external access to several attached actors
    // we can think of this exposed actor as a capability
    // "tor:0139aa9b4d523e1da515ce21a818e579acd005fbd0aea62ef094ac1b845f99e7@someaddress.onion"
    let address = router_handle.attach(untyped_handle);

    let remote_handle = RemoteHandle<u32, u32, SomeError>::new(address)?;
    //                     type handle once ^       parse address ^

    // use RemoteHandle just like a LocalHandle
    let res = remote_handle.send(42)?;

    // capabilities can be revoked anytime
    router_handle.revoke(address);

    // ...and thus we can't invoke this one anymore
    let remote_handle = RemoteHandle<u32, u32, SomeError>::new(address).unwrap_err();
}
```

# License

Copyright 2024 Ariela Wenner

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
