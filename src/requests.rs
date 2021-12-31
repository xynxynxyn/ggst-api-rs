use crate::{error::*, *};
use chrono::{DateTime, NaiveDateTime, Utc};
use hex::ToHex;
use lazy_static::lazy_static;
use regex::{bytes, Regex};
use reqwest;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::str;

const DEFAULT_UTILS_BASE_URL: &str =
    "https://ggst-utils-default-rtdb.europe-west1.firebasedatabase.app";
const DEFAULT_BASE_URL: &str = "https://ggst-game.guiltygear.com";

/// Context struct which contains the base urls used for api requests. Use the associated methods
/// to overwrite urls if necessary.
pub struct Context {
    base_url: String,
    utils_base_url: String,
}

impl Default for Context {
    fn default() -> Self {
        Context {
            base_url: DEFAULT_BASE_URL.to_string(),
            utils_base_url: DEFAULT_UTILS_BASE_URL.to_string(),
        }
    }
}

impl Context {
    pub fn new() -> Self {
        Context::default()
    }

    /// Overwrite the url used for api requests. The default is https://ggst-game.guiltygear.com
    pub fn base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// Overwrite the url used for requests regarding static content, such as user ids. The default
    /// is https://ggst-utils-default-rtdb.europe-west1.firebasedatabase.app
    pub fn utils_base_url(mut self, utils_base_url: String) -> Self {
        self.utils_base_url = utils_base_url;
        self
    }
}

/// Retrieve the latest set of replays. Each page contains approximately 10 replays, however this is not
/// guaranteed. Indicate the min and maximum floor you want to query.
/// No more than 100 pages can be queried at a time.
pub async fn get_replays(
    context: &Context,
    pages: usize,
    min_floor: Floor,
    max_floor: Floor,
) -> Result<HashSet<Match>> {
    if pages > 100 {
        return Err(Error::InvalidArguments(format!(
            "pages: {} Cannot query more than 100 pages",
            pages
        )));
    }
    if min_floor > max_floor {
        return Err(Error::InvalidArguments(format!(
            "min_floor {:?} is larger than max_floor {:?}",
            min_floor, max_floor
        )));
    }

    let request_url = format!("{}/api/catalog/get_replay", context.base_url);
    let client = reqwest::Client::new();

    let mut matches = HashSet::with_capacity(pages * 10);
    for i in 0..pages {
        let hex_index = format!("{:02X}", i);
        let query_string = format!(
            "9295B2323131303237313133313233303038333834AD3631613565643466343631633202A5302E302E38039401CC{}0A9AFF00{}{}90FFFF000001",
            hex_index,
            min_floor.to_hex(),
            max_floor.to_hex());
        let response = client
            .post(&request_url)
            .form(&[("data", query_string)])
            .send()
            .await?;

        // Parse the response
        lazy_static! {
            // This separates the matches from each other
            static ref MATCH_SEP: bytes::Regex =
                //bytes::Regex::new(r"(?-u)\xEF\xBF\xBD\xEF\xBF\xBD\x02\xEF\xBF\xBD\x70\xEF\xBF\xBD")
                bytes::Regex::new(r"(?-u)\x01\x00\x00\x00")
                    .expect("Could not compile regex");
            // The separator which separates data within a match segment
            static ref PLAYER_DATA_START: bytes::Regex = bytes::Regex::new(r"(?-u)\x95\xb2").expect("Could not compile regex");
        }
        // Convert the response to raw bytes
        let bytes = response.bytes().await?;
        // Remove the first 91 bytes, they are static header
        // They are of no use to us
        let bytes = bytes.slice(61..);
        // Split on the match separator and keep non empty results only
        // This should give us 10 separate matches
        for raw_match in MATCH_SEP.split(&bytes).filter(|b| !b.is_empty()) {
            // Structure of the data to be extracted:
            // We have three sections that have to be parsed
            // Section 1: {floor}{p1_char}{p2_char}
            // Section 2: \x95\xb2{p1_id [18 chars]}\xa_{p1_name}\xb1{p1_some_number}\xaf{p1_online_id}\x07
            // Section 3: \x95\xb2{p2_id}\xa_{p2_name}\xb1{p2_some_number}\xaf{p2_online_id}\t{winner}\xb3{timestamp}

            // Split the match data on the player separator
            let mut data = PLAYER_DATA_START.split(raw_match);

            let (floor, p1_char, p2_char) = match data.next() {
                Some(b) => {
                    let n = b.len();
                    if n < 3 {
                        return Err(Error::UnexpectedResponse(
                            "First data part does not have 3 bytes".into(),
                        ));
                    }
                    (b[n - 3], b[n - 2], b[n - 1])
                }
                None => {
                    return Err(Error::UnexpectedResponse(
                        "Could not find first data part of response".into(),
                    ))
                }
            };

            let (p1_id, p1_name) = match data.next() {
                Some(b) => {
                    // We check if the array is long enough
                    // it has to be at least 18 characters for the player user_id
                    // one character for the separator \xa_ and then at least 1 byte for
                    // the username
                    if b.len() < 20 {
                        return Err(Error::UnexpectedResponse(
                            "Second data part does not have 20 bytes".into(),
                        ));
                    }

                    let name = match b[19..].split(|f| *f == b'\xb1').next() {
                        Some(name_bytes) => String::from_utf8_lossy(name_bytes),
                        None => {
                            return Err(Error::UnexpectedResponse(
                                "Could not parse player1 name".into(),
                            ))
                        }
                    };
                    (String::from_utf8_lossy(&b[0..18]), name)
                }
                None => {
                    return Err(Error::UnexpectedResponse(
                        "Could not find second data part of response".into(),
                    ))
                }
            };

            let (p2_id, p2_name, winner, time) = match data.next() {
                Some(b) => {
                    // We check if the array is long enough
                    // it has to be at least 18 characters for the player user_id
                    // one character for the separator \xa_ and then at least 1 byte for
                    // the username
                    if b.len() < 20 {
                        return Err(Error::UnexpectedResponse(
                            "Third data part does not have 20 bytes".into(),
                        ));
                    }

                    let name = match b[19..].split(|f| *f == b'\xb1').next() {
                        Some(name_bytes) => String::from_utf8_lossy(name_bytes),
                        None => {
                            return Err(Error::UnexpectedResponse(
                                "Could not parse player2 name".into(),
                            ))
                        }
                    };

                    let (winner, time) = match b[19..].split(|f| *f == b'\t').nth(1) {
                        Some(bytes) => {
                            // We need 1 byte for the winner, 1 byte for the separator and 19 bytes
                            // for the timestamp
                            if bytes.len() < 21 {
                                return Err(Error::UnexpectedResponse(
                                    "Not enough bytes to parse winner and timestamp bytes".into(),
                                ));
                            }

                            (bytes[0], String::from_utf8_lossy(&bytes[2..21]))
                        }
                        None => {
                            return Err(Error::UnexpectedResponse(
                                "Could not parse winner and timestamp".into(),
                            ))
                        }
                    };
                    (String::from_utf8_lossy(&b[0..18]), name, winner, time)
                }
                None => {
                    return Err(Error::UnexpectedResponse(
                        "Could not find third data part of match".into(),
                    ))
                }
            };

            let match_data = Match {
                floor: Floor::from_u8(floor)?,
                timestamp: match NaiveDateTime::parse_from_str(&time, "%Y-%m-%d %H:%M:%S") {
                    Ok(t) => DateTime::<Utc>::from_utc(t, Utc),
                    Err(_) => {
                        return Err(Error::UnexpectedResponse(format!(
                            "Could not parse datetime {}",
                            &time
                        )))
                    }
                },
                players: (
                    Player {
                        id: p1_id.to_string(),
                        name: p1_name.to_string(),
                        character: Character::from_u8(p1_char)?,
                    },
                    Player {
                        id: p2_id.to_string(),
                        name: p2_name.to_string(),
                        character: Character::from_u8(p2_char)?,
                    },
                ),
                winner: match winner {
                    1 => Winner::Player1,
                    2 => Winner::Player2,
                    _ => {
                        return Err(Error::UnexpectedResponse(format!(
                            "Could not parse winner {}",
                            winner
                        )))
                    }
                },
            };
            matches.insert(match_data);
        }
    }
    Ok(matches)
}

async fn userid_from_steamid(context: &Context, steamid: &str) -> Result<String> {
    let request_url = format!("{}/{}.json", context.utils_base_url, steamid);
    let response = reqwest::get(request_url).await?;
    let d: Value = serde_json::from_str(&response.text().await?)?;
    match d.get("UserID") {
        Some(s) => Ok(String::from(
            s.as_str()
                .ok_or(Error::UnexpectedResponse("Could not parse user id".into()))?,
        )),
        None => Err(Error::UnexpectedResponse("Could not parse user id".into())),
    }
}

pub async fn user_from_steamid(context: &Context, steamid: &str) -> Result<User> {
    // Get the user id from the steamid
    let id = userid_from_steamid(context, steamid).await?;

    // Construct the request with token and appropriate AOB
    let request_url = format!("{}/api/statistics/get", context.base_url);
    let client = reqwest::Client::new();
    let query = format!(
        "9295B2323131303237313133313233303038333834AD3631393064363236383739373702A5302E302E380396B2{}070101FFFFFF",
        id.encode_hex::<String>()
    );
    let response = client
        .post(request_url)
        .form(&[("data", query)])
        .send()
        .await?;

    // Remove invalid unicode stuff before the actual json body
    let content = &response.text().await?;
    lazy_static! {
        static ref RE: Regex = Regex::new(r"[^\{]*\{").expect("Could not compile regex");
    }
    let content = RE.replacen(content, 1, "{");
    let v: Value = serde_json::from_str(&content)?;

    // Assemble the user object
    Ok(User {
        id,
        name: String::from(
            v.get("NickName")
                .ok_or(Error::UnexpectedResponse("Could not parse username".into()))?
                .as_str()
                .ok_or(Error::UnexpectedResponse("Could not parse username".into()))?,
        ),
        comment: String::from(
            v.get("PublicComment")
                .ok_or(Error::UnexpectedResponse(
                    "Could not parse profile comment".into(),
                ))?
                .as_str()
                .ok_or(Error::UnexpectedResponse(
                    "Could not parse profile comment".into(),
                ))?,
        ),
        floor: Floor::Celestial,
        stats: MatchStats { total: 0, wins: 0 },
        celestial_stats: MatchStats { total: 0, wins: 0 },
        char_stats: HashMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn get_userid() {
        let ctx = Context::new();
        let id = userid_from_steamid(&ctx, "76561198045733267")
            .await
            .unwrap();
        assert_eq!(id, "210611132841904307");
    }

    #[tokio::test]
    async fn get_user_stats() {
        let ctx = Context::new();
        let user = user_from_steamid(&ctx, "76561198045733267").await.unwrap();
        assert_eq!(user.name, "enemy fungus");
    }

    #[tokio::test]
    async fn query_replays() {
        let ctx = Context::new();
        let n_replays = 20;
        let replays = get_replays(&ctx, n_replays, Floor::Celestial, Floor::Celestial)
            .await
            .unwrap();
        replays.iter().take(10).for_each(|m| println!("{}", m));
        println!("Got {} replays", replays.len());
    }
}
