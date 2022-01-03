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
    min_floor: Floor,
    max_floor: Floor,
) -> Result<(impl Iterator<Item = Match>, impl Iterator<Item = ParseError>)>
```

## Example

This example fetches 100 pages of at most 127 replays each between floor 7 and celestial.
It then prints the meta data for all replays collected as well as report any parsing errors.

```rust
use ggst-api::*;
let ctx = Context::new();
let (replays, parsing_errors) = get_replays(&ctx, 100, 127, Floor::F7, Floor::Celestial).await.unwrap();
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
    pub fn floor(&self) -> &Floor;
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
The body should be of the content type `application/x-www-form-urlencoded` and contain a single entry with the key data.
Refer to https://github.com/optix2000/totsugeki/issues/35#issuecomment-922516535 for more information about the specific encoding used here.

```
data="9295B2323131303237313133313233303038333834AD3631613565643466343631633202A5302E302E38039401CC{page_index_hex}{replays_per_page_hex}9AFF00{min_floor_hex}{max_floor_hex}90FFFF000001"
```

The response consists of raw bytes, mainly invalid unicode.
Every page begins with a header of about ~60 bytes which is not needed.

This is the structure of the response for each replay on a page:

```
{garbage[..]}{floor[1]}{p1_char[1]}{p2_char[1]}\x95\xb2{p1_id[18]}\xa_{p1_name[..]}\xb1{p1_steam_id?[..]}\xaf{p1_online_id[..]}\x07\x95\xb2{p2_id[18]}\xa_{p2_name[..]}\xb1{p2_steam_id?[..]}\xaf{p2_online_id[..]}\t{winner[1]}\xb3{timestamp[19]}\x01\x00\x00\x00
```

The brackets indicate the length in bytes of the field.
Since after every timestamp there is the '\x01\x00\x00\x00' pattern we can split a page on that sequence.
Then we use the '\x96\xb2' byte sequence to split each match into three sections.

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
