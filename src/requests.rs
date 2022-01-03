use crate::{error::*, *};

use chrono::{DateTime, NaiveDateTime, Utc};
use lazy_static::lazy_static;
use regex::bytes;
use reqwest::{self, header};
use std::collections::BTreeSet;
use std::str;

const DEFAULT_BASE_URL: &str = "https://ggst-game.guiltygear.com";

/// Context struct which contains the base urls used for api requests. Use the associated methods
/// to overwrite urls if necessary.
pub struct Context {
    base_url: String,
}

impl Default for Context {
    fn default() -> Self {
        Context {
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }
}

impl Context {
    pub fn new() -> Self {
        Context::default()
    }

    /// Overwrite the url used for api requests. The default is https://ggst-game.guiltygear.com
    /// You can modify this to a proxy in your area for faster requests
    pub fn base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
}

fn id_from_bytes(bytes: &[u8]) -> Result<i64> {
    let s =
        str::from_utf8(bytes).map_err(|_| Error::ParsingBytesError("could not parse userid"))?;
    Ok(i64::from_str_radix(s, 10)
        .map_err(|_| Error::ParsingBytesError("could not parse userid from String"))?)
}

/// Retrieve the latest set of replays. Each page contains approximately 10 replays by default, however this is not
/// guaranteed. Indicate the min and maximum floor you want to query.
/// No more than 100 pages can be queried at a time and only 127 replays per page max.
/// If no matches can be found the parsing will fail.
/// Usually a few replays have weird timestamps from the future. It is recommended to apply a
/// filter on the current time before using any matches, like `.filter(|m| m.timestamp() < &chrono::Utc::now())`
pub async fn get_replays(
    context: &Context,
    pages: usize,
    replays_per_page: usize,
    min_floor: Floor,
    max_floor: Floor,
) -> Result<(
    impl Iterator<Item = Match>,
    impl Iterator<Item = ParseError>,
)> {
    // Check for invalid inputs
    if pages > 100 {
        return Err(Error::InvalidArgument(format!(
            "cannot query more than 100 pages, queried {}",
            pages
        )));
    }
    if replays_per_page > 127 {
        return Err(Error::InvalidArgument(format!(
            "cannot query more than 127 replays per page, queried {}",
            replays_per_page
        )));
    }
    if min_floor > max_floor {
        return Err(Error::InvalidArgument(format!(
            "min_floor {:?} is larger than max_floor {:?}",
            min_floor, max_floor
        )));
    }

    let request_url = format!("{}/api/catalog/get_replay", context.base_url);
    let client = reqwest::Client::new();

    // Assume at most 10 replays per page for pre allocation
    let mut matches = BTreeSet::new();
    let mut errors = vec![];
    for i in 0..pages {
        // Construct the query string
        let hex_index = format!("{:02X}", i);
        let replays_per_page_hex = format!("{:02X}", replays_per_page);
        let query_string = format!(
            "9295B2323131303237313133313233303038333834AD3631613565643466343631633202A5302E302E38039401CC{}{}9AFF00{}{}90FFFF000001",
            hex_index,
            replays_per_page_hex,
            min_floor.to_hex(),
            max_floor.to_hex());
        let response = client
            .post(&request_url)
            .header(header::USER_AGENT, "Steam")
            .header(header::CACHE_CONTROL, "no-cache")
            .form(&[("data", query_string)])
            .send()
            .await?;

        // Regex's to parse the raw bytes received
        lazy_static! {
            // This separates the matches from each other
            static ref MATCH_SEP: bytes::Regex =
                bytes::Regex::new(r"(?-u)\x01\x00\x00\x00")
                    .expect("Could not compile regex");
        }

        // Convert the response to raw bytes
        let bytes = response.bytes().await?;

        // Check if only the header is present
        // If yes then we found no matches and return early
        // The function should not fail but rather return an empty set or what was already found
        if bytes.len() < 63 {
            return Ok((matches.into_iter(), errors.into_iter()));
        }

        // Remove the first 61 bytes, they are static header, we don't need them
        let bytes = bytes.slice(61..);

        // Split on the match separator and keep non empty results only
        // This should give us 10 separate matches
        for raw_match in MATCH_SEP.split(&bytes).filter(|b| !b.is_empty()) {
            // Insert it into the set
            match parse_match(raw_match) {
                Ok(m) => {
                    matches.insert(m);
                }
                Err(e) => {
                    errors.push(ParseError::new(show_buf(raw_match), e));
                }
            };
        }
    }
    Ok((matches.into_iter(), errors.into_iter()))
}

fn parse_match(raw_match: &[u8]) -> Result<Match> {
    // The separator which separates data within a match segment
    lazy_static! {
        static ref PLAYER_DATA_START: bytes::Regex =
            bytes::Regex::new(r"(?-u)\x95\xb2").expect("Could not compile regex");
    }
    // Structure of the data to be extracted:
    // We have three sections that have to be parsed
    // Section 1: {floor}{p1_char}{p2_char}
    // Section 2: \x95\xb2{p1_id [18 chars]}\xa_{p1_name}\xb1{p1_some_number}\xaf{p1_online_id}\x07
    // Section 3: \x95\xb2{p2_id}\xa_{p2_name}\xb1{p2_some_number}\xaf{p2_online_id}\t{winner}\xb3{timestamp}

    // Split the match data on the player separator
    let mut data = PLAYER_DATA_START
        .split(raw_match)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .take(3)
        .rev();

    // Section 1
    let (floor, p1_char, p2_char) = match data.next() {
        Some(b) => {
            let n = b.len();
            if n < 3 {
                return Err(Error::UnexpectedResponse(
                    "first data part does not have 3 bytes",
                ));
            }
            (b[n - 3], b[n - 2], b[n - 1])
        }
        None => {
            return Err(Error::UnexpectedResponse(
                "could not find first data part of response",
            ))
        }
    };

    // Section 2
    let (p1_id, p1_name) = match data.next() {
        Some(b) => {
            // We check if the array is long enough
            // it has to be at least 18 characters for the player user_id
            // one character for the separator \xa_ and then at least 1 byte for
            // the username
            if b.len() < 20 {
                return Err(Error::UnexpectedResponse(
                    "second data part does not have 20 bytes",
                ));
            }

            let name = match b[19..].split(|f| *f == b'\xb1').next() {
                Some(name_bytes) => String::from_utf8_lossy(name_bytes),
                None => return Err(Error::UnexpectedResponse("could not parse player1 name")),
            };
            (id_from_bytes(&b[0..18])?, name)
        }
        None => {
            return Err(Error::UnexpectedResponse(
                "could not find second data part of response",
            ))
        }
    };

    // Section 3
    let (p2_id, p2_name, winner, time) = match data.next() {
        Some(b) => {
            // We check if the array is long enough, 76 characters required for a 1 byte
            // username, it has to be at least 76 characters for the player user_id, online_id,
            // timestamp, the other number and the winner indicator and separators
            // and then at least 1 byte for the username
            // There do exist weird edge cases where the third data part does not contain
            // an online id, instead it has a dummy user name, this will then take 71 bytes
            // instead
            if b.len() < 71 {
                return Err(Error::UnexpectedResponse(
                    "third data part does not have 71 bytes",
                ));
            }

            let name = match b[19..].split(|f| *f == b'\xb1').next() {
                Some(name_bytes) => String::from_utf8_lossy(name_bytes),
                None => return Err(Error::UnexpectedResponse("could not find player2 name")),
            };

            // first 38 bytes are unnecessary as they contain the username and id's
            // \xb3 is in front of the timestamp, so we split the bytes on that and take
            // the last two segements, which should be the winner and timestamp
            // This can break if there are more bytes behind the timestamp that contain the
            // \xb3 byte
            let winner_time_bytes = b[38..]
                .split(|f| *f == b'\xb3')
                .rev()
                .take(2)
                .collect::<Vec<_>>();
            let time = match winner_time_bytes.get(0) {
                Some(b) => {
                    // 16 bytes before the relevant section
                    // We need 1 byte for the winner, 1 byte for the separator and 19 bytes
                    // for the timestamp
                    if b.len() < 19 {
                        return Err(Error::UnexpectedResponse(
                            "not enough bytes to parse timestamp",
                        ));
                    }
                    String::from_utf8_lossy(&b[0..19])
                }
                None => {
                    return Err(Error::UnexpectedResponse(
                        "could not split bytes to parse winner and timestamp",
                    ))
                }
            };
            let winner = match winner_time_bytes.get(1) {
                Some(b) => match b.last() {
                    None => {
                        return Err(Error::UnexpectedResponse("could not find winner in bytes"))
                    }
                    Some(b) => b,
                },
                None => {
                    return Err(Error::UnexpectedResponse(
                        "could not split bytes to parse winner",
                    ))
                }
            };
            //(id_from_bytes(&b[0..18])?, name, winner, time)
            (id_from_bytes(&b[0..18])?, name, winner, time)
        }
        None => {
            return Err(Error::UnexpectedResponse(
                "could not find third data part of match",
            ))
        }
    };

    // Construct the match
    let m = Match {
        floor: Floor::from_u8(floor)?,
        timestamp: DateTime::<Utc>::from_utc(
            NaiveDateTime::parse_from_str(&time, "%Y-%m-%d %H:%M:%S")?,
            Utc,
        ),
        players: (
            Player {
                id: p1_id,
                name: p1_name.to_string(),
                character: Character::from_u8(p1_char)?,
            },
            Player {
                id: p2_id,
                name: p2_name.to_string(),
                character: Character::from_u8(p2_char)?,
            },
        ),
        winner: match winner {
            1 => Winner::Player1,
            2 => Winner::Player2,
            _ => return Err(Error::ParsingBytesError("Could not parse winner")),
        },
    };

    Ok(m)
}

// Helper function for constructing error messages to avoid issues with the borrow checker
fn show_buf<B: AsRef<[u8]>>(buf: B) -> String {
    use std::ascii::escape_default;
    String::from_utf8(
        buf.as_ref()
            .iter()
            .map(|b| escape_default(*b))
            .flatten()
            .collect(),
    )
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn query_replays() {
        let ctx = Context::new();
        let n_replays = 100;
        let n_replays_per_page = 127;
        let (replays, errors) = get_replays(
            &ctx,
            n_replays,
            n_replays_per_page,
            Floor::F1,
            Floor::Celestial,
        )
        .await
        .unwrap();
        let replays = replays
            .filter(|m| m.timestamp() < &Utc::now())
            .collect::<Vec<_>>();
        println!("Got {} replays", replays.len());
        if replays.len() > 1 {
            println!("Oldest replay: {}", replays.first().unwrap());
            println!("Latest replay: {}", replays.last().unwrap());
        }

        println!("Errors:");
        let errors = errors
            .map(|e| {
                eprintln!("{}", e);
                e
            })
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 0);
    }
}
