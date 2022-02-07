# ggst-api-rs

An async library for interfacing with the replay REST API of Guilty Gear Strive

Use the get_replays function to collect the latest replays from the AWS servers.
A maximum of 100 pages can be queried at a time with up to 127 replays per page.
Note that it is not guaranteed that a page is full since duplicates are removed and this may result in less than the expected number.
```rust
pub async fn get_replays(
    context: &Context,
    pages: usize,
    replays_per_page: usize,
    query_parameters: QueryParameters,
) -> Result<(impl Iterator<Item = Match>, impl Iterator<Item = ParseError>)>
```

## Example

This example fetches 100 pages of at most 127 replays each between floor 7 and celestial where Sol
is present.
It then prints the meta data for all replays collected as well as report any parsing errors.

```rust
use ggst_api::*;
let (replays, parsing_errors) = get_replays(
    &Context::default(),
    100,
    127,
    QueryParameters::default()
        .min_floor(Floor::F7)
        .max_floor(Floor::Celestial)
        .character(Character::Sol)
    ).await.unwrap();
println!("Replays:");
replays.for_all(|r| println!("{}", r));
println!("Errors:");
parsing_errors.for_all(|e| println!("{}", e));
```

## Structs

The main two structs are `Match` and `Player` with the following interfaces.
They implement common traits such as `Eq`, `Hash`, `Clone`, `Display` etc.
The `Eq` implementation for `Player` ignores the name value as a user can change their nickname all the time while their id stays the same.

```rust
pub struct Match;
impl Match {
    pub fn floor(&self) -> Floor;
    pub fn timestamp(&self) -> &DateTime<Utc>;
    pub fn players(&self) -> (&Player, &Player);
    pub fn winner(&self) -> &Player;
    pub fn loser(&self) -> &Player;
}

pub struct Player;
impl Player {
    pub fn id(&self) -> u64;
    pub fn name(&self) -> &str;
    pub fn character(&self) -> Character;
}
```

## How does the API work?

To collect replays a POST request has to be made to https://ggst-game.guiltygear.com/api/catalog/get_replay.
The body should be of the content type `application/x-www-form-urlencoded` and contain a single entry with the key data which is
hex encoded [messagepack](https://msgpack.org/). The response is plain messagepack. Rust types are defined for both the request and response
with all know fields having readable names.

## Why does it return an error?
Sometimes the response is malformed and a replay cannot be parsed.
Usually this happens because one of the two usernames in a match uses unicode characters that contain some of the byte sequences used to split the response.
If this happens the error message will show the raw bytes, please open an issue and show the data that cannot be parsed.

## Features

Enable the serde feature for serialization support.
```toml
[dependencies]
ggst-api = { path = "./ggst-api", features = ["serde"] }
```
