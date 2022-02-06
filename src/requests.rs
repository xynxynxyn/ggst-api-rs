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
    /// Overwrite the url used for api requests. The default is https://ggst-game.guiltygear.com
    /// You can modify this to a proxy in your area for faster requests
    pub fn new(base_url: String) -> Self {
        Context { base_url }
    }
}

fn id_from_bytes(bytes: &[u8]) -> Result<i64> {
    let s =
        str::from_utf8(bytes).map_err(|_| Error::ParsingBytesError("could not parse userid"))?;
    s.parse::<i64>()
        .map_err(|_| Error::ParsingBytesError("could not parse userid from String"))
}

/// Retrieve the latest set of replays. Each page contains approximately 10 replays by default, however this is not
/// guaranteed. Indicate the min and maximum floor you want to query.
/// No more than 100 pages can be queried at a time and only 127 replays per page max.
/// If no matches can be found the parsing will fail.
/// Usually a few replays have weird timestamps from the future. It is recommended to apply a
/// filter on the current time before using any matches, like `.filter(|m| m.timestamp() < &chrono::Utc::now())`
pub async fn get_replays<A, B, C, D, E>(
    context: &Context,
    pages: usize,
    replays_per_page: usize,
    request_parameters: QueryParameters<A, B, C, D, E>,
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

    if request_parameters.min_floor > request_parameters.max_floor {
        return Err(Error::InvalidArgument(format!(
            "min_floor {:?} is larger than max_floor {:?}",
            request_parameters.min_floor, request_parameters.max_floor
        )));
    }

    let request_url = format!("{}/api/catalog/get_replay", context.base_url);
    let client = reqwest::Client::new();

    // Assume at most 10 replays per page for pre allocation
    let mut matches = BTreeSet::new();
    let mut errors = vec![];
    for i in 0..pages {
        // Construct the query string
        let query_string = messagepack::ReplayRequest {
            header: messagepack::RequestHeader {
                string1: "211027113123008384".into(),
                string2: "61a5ed4f461c2".into(),
                int1: 2,
                version: "0.1.0".into(),
                int2: 3,
            },
            body: messagepack::RequestBody {
                int1: 1,
                index: i,
                replays_per_page,
                query: messagepack::RequestQuery::from(&request_parameters),
            },
        }
        .to_hex();
        let response = client
            .post(&request_url)
            .header(header::USER_AGENT, "Steam")
            .header(header::CACHE_CONTROL, "no-cache")
            .form(&[("data", query_string)])
            .send()
            .await?;

        // Convert the response to raw bytes
        let bytes = response.bytes().await?;

        if !parse_response(&mut matches, &mut errors, &bytes) {
            return Ok((matches.into_iter(), errors.into_iter()));
        }
    }
    Ok((matches.into_iter(), errors.into_iter()))
}

fn parse_messagepack_response(
    matches: &mut BTreeSet<Match>,
    errors: &mut Vec<ParseError>,
    bytes: &[u8],
) -> bool {
    match rmp_serde::decode::from_slice::<messagepack::ReplayResponse>(bytes) {
        Ok(response) => {
            for replay in response.body.replays {
                match match_from_replay(replay.clone()) {
                    Ok(m) => {
                        matches.insert(m);
                    }
                    Err(e) => {
                        errors.push(ParseError::new(show_buf(bytes), e));
                    }
                }
            }
        }
        Err(e) => {
            errors.push(ParseError::new(show_buf(bytes), e.into()));
        }
    }

    true
}

fn match_from_replay(replay: messagepack::Replay) -> Result<Match> {
    Ok(Match {
        floor: Floor::from_u8(replay.floor)?,
        timestamp: replay.date,
        players: (
            Player::try_from((replay.player1_character, replay.player1))?,
            Player::try_from((replay.player2_character, replay.player2))?,
        ),
        winner: match replay.winner {
            1 => Winner::Player1,
            2 => Winner::Player2,
            _ => return Err(Error::ParsingBytesError("Could not parse winner")),
        },
    })
}

impl TryFrom<(Character, messagepack::Player)> for Player {
    type Error = Error;
    fn try_from((character, player): (Character, messagepack::Player)) -> Result<Self> {
        Ok(Player {
            id: id_from_bytes(player.id.as_bytes())?,
            name: player.name,
            character,
        })
    }
}

fn parse_response(
    matches: &mut BTreeSet<Match>,
    errors: &mut Vec<ParseError>,
    bytes: &[u8],
) -> bool {
    // Regex's to parse the raw bytes received
    lazy_static! {
        // This separates the matches from each other
        static ref MATCH_SEP: bytes::Regex =
            bytes::Regex::new(r"(?-u)\x01\x00\x00\x00")
                .expect("Could not compile regex");
    }

    // Check if only the header is present
    // If yes then we found no matches and return early
    // The function should not fail but rather return an empty set or what was already found
    if bytes.len() < 63 {
        return false;
    }

    // Remove the first 61 bytes, they are static header, we don't need them
    let bytes = &bytes[61..];

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

    true
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
            .flat_map(|b| escape_default(*b))
            .collect(),
    )
    .unwrap()
}

mod messagepack {
    use super::*;

    use serde_crate::{
        de::{Deserializer, Error as _},
        Deserialize,
    };

    use crate::Character;

    impl ReplayRequest {
        pub fn to_hex(&self) -> String {
            use std::fmt::Write;

            let mut buf = String::new();
            for b in rmp_serde::encode::to_vec(self).unwrap() {
                write!(buf, "{:02X}", b).unwrap();
            }
            buf
        }
    }

    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "serde", derive(Serialize), serde(crate = "serde_crate"))]
    pub struct ReplayRequest {
        pub header: RequestHeader,
        pub body: RequestBody,
    }

    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "serde", derive(Serialize), serde(crate = "serde_crate"))]
    pub struct RequestHeader {
        pub string1: String,
        pub string2: String,
        pub int1: i32,
        pub version: String,
        pub int2: i32,
    }

    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "serde", derive(Serialize), serde(crate = "serde_crate"))]
    pub struct RequestBody {
        pub int1: u8,
        pub index: usize,
        pub replays_per_page: usize,
        pub query: RequestQuery,
    }

    impl<A, B, C, D, E> From<&QueryParameters<A, B, C, D, E>> for RequestQuery {
        fn from(query: &QueryParameters<A, B, C, D, E>) -> Self {
            RequestQuery {
                int1: -1,
                int2: 0,
                min_floor: query.min_floor.to_u8(),
                max_floor: query.max_floor.to_u8(),
                seq: vec![],
                char_1: query.char_1.map_or_else(|| -1, |c| c.to_u8() as i8),
                char_2: query.char_2.map_or_else(|| -1, |c| c.to_u8() as i8),
                winner: query.winner.map_or_else(
                    || 0x00,
                    |w| match w {
                        Winner::Player1 => 0x01,
                        Winner::Player2 => 0x02,
                    },
                ),
                int8: 0,
                int9: 1,
            }
        }
    }

    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "serde", derive(Serialize), serde(crate = "serde_crate"))]
    pub struct RequestQuery {
        pub int1: i8,
        pub int2: u8,
        pub min_floor: u8,
        pub max_floor: u8,
        pub seq: Vec<()>,
        pub char_1: i8,
        pub char_2: i8,
        pub winner: u8,
        pub int8: u8,
        pub int9: u8,
    }
    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "serde", derive(Deserialize), serde(crate = "serde_crate"))]
    pub struct ReplayResponse {
        pub header: ResponseHeader,
        pub body: ResponseBody,
    }
    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "serde", derive(Deserialize), serde(crate = "serde_crate"))]
    pub struct ResponseHeader {
        pub id: String,
        pub int1: i32,
        pub date: String,
        pub version1: String,
        pub version2: String,
        pub version3: String,
        pub string1: String,
        pub string2: String,
    }

    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "serde", derive(Deserialize), serde(crate = "serde_crate"))]
    pub struct ResponseBody {
        pub int1: i32,
        pub int2: i32,
        pub int3: i32,
        pub replays: Vec<Replay>,
    }
    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "serde", derive(Deserialize), serde(crate = "serde_crate"))]
    pub struct Replay {
        pub int1: u64,
        pub int2: i32,
        pub floor: u8,
        pub player1_character: Character,
        pub player2_character: Character,
        pub player1: Player,
        pub player2: Player,
        pub winner: u8,

        #[serde(deserialize_with = "deserialize_date_time")]
        pub date: chrono::DateTime<Utc>,
        pub int7: i32,
        pub int8: i32,
        pub int9: i32,
        pub int10: i32,
    }

    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "serde", derive(Deserialize), serde(crate = "serde_crate"))]
    pub struct Player {
        pub id: String,
        pub name: String,
        pub string1: String,
        pub string2: String,
        pub int1: i32,
    }

    fn deserialize_date_time<'de, D>(
        deserializer: D,
    ) -> std::result::Result<chrono::DateTime<chrono::Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let time = String::deserialize(deserializer)?;
        Ok(DateTime::<Utc>::from_utc(
            NaiveDateTime::parse_from_str(&time, "%Y-%m-%d %H:%M:%S").map_err(D::Error::custom)?,
            Utc,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response() {
        const RESPONSE: &[u8] = b"\x92\x98\xad61ff0796545a9\0\xb32022/02/05 23:26:14\xa50.1.0\xa50.0.2\xa50.0.2\xa0\xa0\x94\0\0\x1e\xdc\0\x1e\x9d\xcf\x03\x0eS}\x9f\x8ds\xbf\t\x08\x0c\x0b\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x95\xb2210818223745601103\xafSamuraiPizzaCat\xb176561199149925226\xaf110000146e8c36a\x07\x02\xb32022-02-06 04:07:59\x01\0\0\0\x9d\xcf\x03\x0eS|v\xbc6N\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:58:19\x01\0\0\0\x9d\xcf\x03\x0eS|lr}\xc1\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:56:46\x01\0\0\0\x9d\xcf\x03\x0eS|du\xac>\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:55:12\x01\0\0\0\x9d\xcf\x03\x0eSy?\x93\x83\x86\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x01\xb32022-02-06 03:29:31\x01\0\0\0\x9d\xcf\x03\x0eSy/\xfbL\xaa\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x01\xb32022-02-06 03:27:10\x01\0\0\0\x9d\xcf\x03\x0eSy\"\xfc\x1d\x85\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x02\xb32022-02-06 03:24:52\x01\0\0\0\x9d\xcf\x03\x0eSx\xf9\x8c\xd2\r\t\x06\x04\x12\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2210719021019879063\xa9Sebastard\xb176561198354593280\xaf11000011780f600\x05\x01\xb32022-02-06 03:17:56\x01\0\0\0\x9d\xcf\x03\x0eSx\xedf\x1f\xf4\t\x06\x04\x12\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2210719021019879063\xa9Sebastard\xb176561198354593280\xaf11000011780f600\x05\x01\xb32022-02-06 03:15:53\x01\0\0\0\x9d\xcf\x03\x0eS{q&\x8d\x92\t\x07\x05\x0c\x95\xb2220117205818084945\xa8Bugabalu\xb176561198136737187\xaf11000010a84bda3\x05\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x02\xb32022-02-06 03:14:30\x01\0\0\0\x9d\xcf\x03\x0eSx\xe0+\xf8\xf7\t\x06\x04\x12\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2210719021019879063\xa9Sebastard\xb176561198354593280\xaf11000011780f600\x05\x02\xb32022-02-06 03:13:31\x01\0\0\0\x9d\xcf\x03\x0eS{c\xba\xc9z\t\x07\x05\x0c\x95\xb2220117205818084945\xa8Bugabalu\xb176561198136737187\xaf11000010a84bda3\x05\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:12:05\x01\0\0\0\x9d\xcf\x03\x0eS{T\xd4\\\x90\t\x07\x05\x0c\x95\xb2220117205818084945\xa8Bugabalu\xb176561198136737187\xaf11000010a84bda3\x05\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x02\xb32022-02-06 03:09:55\x01\0\0\0\x9d\xcf\x03\x0eS{Ab\xacm\t\x07\x0c\t\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x95\xb2210811193631829778\xaeF4ulty_R4ilgun\xb176561198351152593\xaf1100001174c75d1\x06\x02\xb32022-02-06 03:06:29\x01\0\0\0\x9d\xcf\x03\x0eS{3\xde\xb6\xa2\t\x07\x0c\t\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x95\xb2210811193631829778\xaeF4ulty_R4ilgun\xb176561198351152593\xaf1100001174c75d1\x06\x01\xb32022-02-06 03:04:02\x01\0\0\0\x9d\xcf\x03\x0eS{)\x03G\xe2\t\x07\x0c\t\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x95\xb2210811193631829778\xaeF4ulty_R4ilgun\xb176561198351152593\xaf1100001174c75d1\x06\x02\xb32022-02-06 03:02:20\x01\0\0\0\x9d\xcf\x03\x0eS}\xfct\x97\x16\t\x08\0\x12\x95\xb2210615035914519825\xa5BL4DE\xb176561199083465035\xaf110000142f2a94b\x07\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x01\xb32022-02-06 02:24:18\x01\0\0\0\x9d\xcf\x03\x0eS}\xf3\xeb\x0c\x8a\t\x08\0\x12\x95\xb2210615035914519825\xa5BL4DE\xb176561199083465035\xaf110000142f2a94b\x07\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x02\xb32022-02-06 02:22:34\x01\0\0\0\x9d\xcf\x03\x0eS}\xdb{XM\tc\0\x0e\x95\xb2210611113829735658\xa3Eli\xb176561198449379262\xaf11000011d2747be\t\x95\xb2210612045332227791\xa8R34 I-NO\xb176561198046971684\xaf1100001052b0724\t\x02\xb32022-02-06 02:22:08\x01\0\0\0\x9d\xcf\x03\x0eSy?\xd2\x135\tc\0\x07\x95\xb2210611092701986372\xa3tms\xb176561198223056552\xaf11000010fa9dea8\t\x95\xb2210611184101935607\xb0Shaco Arrombardo\xb176561198019472843\xaf110000103876dcb\t\x02\xb32022-02-06 02:19:53\x01\0\0\0\x9d\xcf\x03\x0eS}\xca\xaeev\tc\0\x0e\x95\xb2210611113829735658\xa3Eli\xb176561198449379262\xaf11000011d2747be\t\x95\xb2210612045332227791\xa8R34 I-NO\xb176561198046971684\xaf1100001052b0724\t\x02\xb32022-02-06 02:19:26\x01\0\0\0\x9d\xcf\x03\x0eSy0\x12\xfd\x84\tc\0\x07\x95\xb2210611092701986372\xa3tms\xb176561198223056552\xaf11000010fa9dea8\t\x95\xb2210611184101935607\xb0Shaco Arrombardo\xb176561198019472843\xaf110000103876dcb\t\x01\xb32022-02-06 02:17:29\x01\0\0\0\x9d\xcf\x03\x0eSy$#\xb0\xfc\tc\0\x07\x95\xb2210611092701986372\xa3tms\xb176561198223056552\xaf11000010fa9dea8\t\x95\xb2210611184101935607\xb0Shaco Arrombardo\xb176561198019472843\xaf110000103876dcb\t\x01\xb32022-02-06 02:15:28\x01\0\0\0\x9d\xcf\x03\x0eS}\xc5\x15\xcf\xf1\t\x08\x12\x12\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x95\xb2210611172901281375\xa4g5h3\xb176561198066767737\xaf110000106591779\x07\x02\xb32022-02-06 02:14:49\x01\0\0\0\x9d\xcf\x03\x0eS}\xb9w\xc3_\t\x08\x12\x12\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x95\xb2210611172901281375\xa4g5h3\xb176561198066767737\xaf110000106591779\x07\x01\xb32022-02-06 02:12:53\x01\0\0\0\x9d\xcf\x03\x0eS}\x95\x1a\x14\xd0\tc\r\0\x95\xb2210611163406897038\xabKidSusSauce\xb176561198796113273\xaf110000131d20579\t\x95\xb2210611113829735658\xa3Eli\xb176561198449379262\xaf11000011d2747be\t\x01\xb32022-02-06 02:10:27\x01\0\0\0\x9d\xcf\x03\x0eS}\xa7$\x04\x91\t\x08\x12\x12\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x95\xb2210611172901281375\xa4g5h3\xb176561198066767737\xaf110000106591779\x07\x01\xb32022-02-06 02:09:46\x01\0\0\0\x9d\xcf\x03\x0eS|x.;\xd4\tc\x01\0\x95\xb2210612195532158554\xa7Nowhere\xb176561198108655731\xaf110000108d84073\t\x95\xb2210611113829735658\xa3Eli\xb176561198449379262\xaf11000011d2747be\t\x02\xb32022-02-06 02:02:47\x01\0\0\0\x9d\xcf\x03\x0eS}re;\xfc\t\x08\x12\x07\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x95\xb2211222194227494329\xacEpicKittyCat\xb176561198040006360\xaf110000104c0bed8\x07\x01\xb32022-02-06 02:01:01\x01\0\0\0\x9d\xcf\x03\x0eS|d\xdd\x9d\x8c\t\x08\x02\x12\x95\xb2211224234141126253\xa6Fakuto\xb176561198387121965\xaf110000119714f2d\x07\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x02\xb32022-02-06 01:55:39\x01\0\0\0";
        let mut matches = BTreeSet::new();
        let mut errors = Vec::new();
        parse_messagepack_response(&mut matches, &mut errors, &RESPONSE);

        assert!(errors.is_empty(), "Got errors: {:#?}", errors);

        expect_test::expect_file!["../test_data/replay_response.txt"].assert_debug_eq(&matches);
    }

    #[test]
    fn test_parse_response_2() {
        // This test used to miss one replay before true messagepack parsing
        const RESPONSE: &[u8] = b"\x92\x98\xad61ff0f60da094\0\xb32022/02/05 23:59:28\xa50.1.0\xa50.0.2\xa50.0.2\xa0\xa0\x94\0\0\n\x9a\x9d\xcf\x03\x0eS}\x9f\x8ds\xbf\t\x08\x0c\x0b\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x95\xb2210818223745601103\xafSamuraiPizzaCat\xb176561199149925226\xaf110000146e8c36a\x07\x02\xb32022-02-06 04:07:59\x01\0\0\0\x9d\xcf\x03\x0eS|v\xbc6N\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:58:19\x01\0\0\0\x9d\xcf\x03\x0eS|lr}\xc1\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:56:46\x01\0\0\0\x9d\xcf\x03\x0eS|du\xac>\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:55:12\x01\x01\0\0\x9d\xcf\x03\x0eSy?\x93\x83\x86\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x01\xb32022-02-06 03:29:31\x01\0\0\0\x9d\xcf\x03\x0eSy/\xfbL\xaa\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x01\xb32022-02-06 03:27:10\x01\0\0\0\x9d\xcf\x03\x0eSy\"\xfc\x1d\x85\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x02\xb32022-02-06 03:24:52\x01\0\0\0\x9d\xcf\x03\x0eSx\xf9\x8c\xd2\r\t\x06\x04\x12\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2210719021019879063\xa9Sebastard\xb176561198354593280\xaf11000011780f600\x05\x01\xb32022-02-06 03:17:56\x01\0\0\0\x9d\xcf\x03\x0eSx\xedf\x1f\xf4\t\x06\x04\x12\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2210719021019879063\xa9Sebastard\xb176561198354593280\xaf11000011780f600\x05\x01\xb32022-02-06 03:15:53\x01\0\0\0\x9d\xcf\x03\x0eS{q&\x8d\x92\t\x07\x05\x0c\x95\xb2220117205818084945\xa8Bugabalu\xb176561198136737187\xaf11000010a84bda3\x05\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x02\xb32022-02-06 03:14:30\x01\0\0\0";

        let mut matches = BTreeSet::new();
        let mut errors = Vec::new();
        parse_messagepack_response(&mut matches, &mut errors, &RESPONSE);

        assert!(errors.is_empty(), "Got errors: {:#?}", errors);

        expect_test::expect_file!["../test_data/replay_response_2.txt"].assert_debug_eq(&matches);
    }

    #[test]
    fn test_query() {
        use messagepack::*;

        let query = ReplayRequest {
            header: RequestHeader {
                string1: "211027113123008384".into(),
                string2: "61a5ed4f461c2".into(),
                int1: 2,
                version: "0.1.0".into(),
                int2: 3,
            },
            body: RequestBody {
                int1: 1,
                index: 0,
                replays_per_page: 127,
                query: RequestQuery {
                    int1: -1,
                    int2: 0,
                    min_floor: 1,
                    max_floor: 99,
                    seq: vec![],
                    char_1: -1,
                    char_2: -1,
                    winner: 0,
                    int8: 0,
                    int9: 1,
                },
            },
        };

        expect_test::expect![["9295B2323131303237313133313233303038333834AD3631613565643466343631633202A5302E312E30039401007F9AFF00016390FFFF000001"]].assert_eq(&query.to_hex())
    }
}
