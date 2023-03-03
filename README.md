# Valence Playground

A minecraft server implementation based on [Valence](https://github.com/valence-rs/valence).

This implementation combines and extends some of the examples provided by the Valence developers, mainly [Terrain generation](https://github.com/valence-rs/valence/blob/main/crates/valence/examples/terrain.rs). I added pregeneration of chunks as well as caching to increase performance. As well as a gui where you can edit some of the settings involved in terrain generation. Building is also implemented but is not yet persistent.

## Running the sever

After cloning the repository, create a copy of [`default.env`](default.env) and rename it to `.env`. Afterwards run

```bash
cargo r -r
```

This will create a new config file and start the server (with a gui). You can disable opening the gui in the config file or by building / running the server without the gui feature flag

```bash
cargo r -r --no-default-features --features minecraft
```

or if you have [cargo-make](https://github.com/sagiegurari/cargo-make) installed

```bash
cargo make no_gui
```

## Configuration options

### `gui`

Enables or disables the gui, when compiled with gui support, otherwise it does nothing

### Worldgeneration

- `seed`: Possible values (default: `"Random"`)
  - `"Random"`: Generates a new seed everytime the server is started
  - `{ Set = u32 }`: Sets the seed to a specific value
- `chunks_cached`: Number of chunks getting cached (defualt: `4000`, a rectangle with about 32 chunks in each direction )
- `spawn`: If set, will be be the spawn point for players (format: `[x, y, z]`, _optional_), otherwise spawn will be one the first block that is not air, with `x=0` and `z=0`
- `prege_chunks`: Area of chunks you want to pregenerate (default: `start = -22, end = 122`)
  - **Important:** Chunk Coordinate `(0, 0)` needs to be in that range (if you dont't specifiy a specific spawn point)

### Server

- `max_connections`: Maximum amount of player connections (default: `20`)
- `max_view_distance`: Maximum view distantce (default: `20`), should 2 chunks less than pre generated chunks for better login experience
- `connection_mode`:
  - `"Online"`:
    > The "online mode" fetches all player data (username, UUID, and skin) from mojangs session server and enables encryption.
    >
    > This mode should be used by all publicly exposed servers which are not behind a proxy.
  - `"OnlineNoProxy"`: Prevents proxy connections, otherwise the same as `"Online"`
  - `"Offline"`:
    > Disables client authentication with the configured session server. Clients can join with any username and UUID they choose, potentially gaining privileges they would not otherwise have. Additionally, encryption is disabled and Minecraft's default skins will be used.
    >
    > This mode should be used for development purposes only and not for publicly exposed servers.
  - `"Velocity"`:
    > This mode is used when the server is behind a Velocity proxy configured with the forwarding mode modern.
    >
    > All player data (username, UUID, and skin) is fetched from the proxy and all connections originating from outside Velocity are blocked.
