use crate::{error::*, *};

use chrono::{DateTime, NaiveDateTime, Utc};
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

fn parse_response(
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
mod messagepack {
    use super::*;

    use serde_crate::{
        de::{Deserializer, Error as _},
        Deserialize,
    };

    use crate::Character;

    // An integer that we don't know the purpose of in the format. Signed and large to prevent unexpectedly large values from causing errors
    type UnknownInteger = i64;

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

    #[derive(Debug, Clone, Serialize)]
    #[serde(crate = "serde_crate")]
    pub struct ReplayRequest {
        pub header: RequestHeader,
        pub body: RequestBody,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(crate = "serde_crate")]
    pub struct RequestHeader {
        pub string1: String,
        pub string2: String,
        pub int1: UnknownInteger,
        pub version: String,
        pub int2: UnknownInteger,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(crate = "serde_crate")]
    pub struct RequestBody {
        pub int1: UnknownInteger,
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

    #[derive(Debug, Clone, Serialize)]
    #[serde(crate = "serde_crate")]
    pub struct RequestQuery {
        pub int1: UnknownInteger,
        pub int2: UnknownInteger,
        pub min_floor: u8,
        pub max_floor: u8,
        pub seq: Vec<()>,
        pub char_1: i8,
        pub char_2: i8,
        pub winner: u8,
        pub int8: UnknownInteger,
        pub int9: UnknownInteger,
    }
    #[derive(Debug, Clone, Deserialize)]
    #[serde(crate = "serde_crate")]
    pub struct ReplayResponse {
        pub header: ResponseHeader,
        pub body: ResponseBody,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(crate = "serde_crate")]
    pub struct ResponseHeader {
        pub id: String,
        pub int1: UnknownInteger,
        pub date: String,
        pub version1: String,
        pub version2: String,
        pub version3: String,
        pub string1: String,
        pub string2: String,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(crate = "serde_crate")]
    pub struct ResponseBody {
        pub int1: UnknownInteger,
        pub int2: UnknownInteger,
        pub int3: UnknownInteger,
        pub replays: Vec<Replay>,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(crate = "serde_crate")]
    pub struct Replay {
        pub int1: u64,
        pub int2: UnknownInteger,
        pub floor: u8,
        pub player1_character: Character,
        pub player2_character: Character,
        pub player1: Player,
        pub player2: Player,
        pub winner: u8,

        #[serde(deserialize_with = "deserialize_date_time")]
        pub date: chrono::DateTime<Utc>,
        pub int7: UnknownInteger,
        pub int8: UnknownInteger,
        pub int9: UnknownInteger,
        pub int10: UnknownInteger,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(crate = "serde_crate")]
    pub struct Player {
        pub id: String,
        pub name: String,
        pub string1: String,
        pub string2: String,
        pub int1: UnknownInteger,
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response() {
        const RESPONSE: &[u8] = b"\x92\x98\xad61ff0796545a9\0\xb32022/02/05 23:26:14\xa50.1.0\xa50.0.2\xa50.0.2\xa0\xa0\x94\0\0\x1e\xdc\0\x1e\x9d\xcf\x03\x0eS}\x9f\x8ds\xbf\t\x08\x0c\x0b\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x95\xb2210818223745601103\xafSamuraiPizzaCat\xb176561199149925226\xaf110000146e8c36a\x07\x02\xb32022-02-06 04:07:59\x01\0\0\0\x9d\xcf\x03\x0eS|v\xbc6N\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:58:19\x01\0\0\0\x9d\xcf\x03\x0eS|lr}\xc1\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:56:46\x01\0\0\0\x9d\xcf\x03\x0eS|du\xac>\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:55:12\x01\0\0\0\x9d\xcf\x03\x0eSy?\x93\x83\x86\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x01\xb32022-02-06 03:29:31\x01\0\0\0\x9d\xcf\x03\x0eSy/\xfbL\xaa\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x01\xb32022-02-06 03:27:10\x01\0\0\0\x9d\xcf\x03\x0eSy\"\xfc\x1d\x85\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x02\xb32022-02-06 03:24:52\x01\0\0\0\x9d\xcf\x03\x0eSx\xf9\x8c\xd2\r\t\x06\x04\x12\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2210719021019879063\xa9Sebastard\xb176561198354593280\xaf11000011780f600\x05\x01\xb32022-02-06 03:17:56\x01\0\0\0\x9d\xcf\x03\x0eSx\xedf\x1f\xf4\t\x06\x04\x12\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2210719021019879063\xa9Sebastard\xb176561198354593280\xaf11000011780f600\x05\x01\xb32022-02-06 03:15:53\x01\0\0\0\x9d\xcf\x03\x0eS{q&\x8d\x92\t\x07\x05\x0c\x95\xb2220117205818084945\xa8Bugabalu\xb176561198136737187\xaf11000010a84bda3\x05\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x02\xb32022-02-06 03:14:30\x01\0\0\0\x9d\xcf\x03\x0eSx\xe0+\xf8\xf7\t\x06\x04\x12\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2210719021019879063\xa9Sebastard\xb176561198354593280\xaf11000011780f600\x05\x02\xb32022-02-06 03:13:31\x01\0\0\0\x9d\xcf\x03\x0eS{c\xba\xc9z\t\x07\x05\x0c\x95\xb2220117205818084945\xa8Bugabalu\xb176561198136737187\xaf11000010a84bda3\x05\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:12:05\x01\0\0\0\x9d\xcf\x03\x0eS{T\xd4\\\x90\t\x07\x05\x0c\x95\xb2220117205818084945\xa8Bugabalu\xb176561198136737187\xaf11000010a84bda3\x05\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x02\xb32022-02-06 03:09:55\x01\0\0\0\x9d\xcf\x03\x0eS{Ab\xacm\t\x07\x0c\t\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x95\xb2210811193631829778\xaeF4ulty_R4ilgun\xb176561198351152593\xaf1100001174c75d1\x06\x02\xb32022-02-06 03:06:29\x01\0\0\0\x9d\xcf\x03\x0eS{3\xde\xb6\xa2\t\x07\x0c\t\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x95\xb2210811193631829778\xaeF4ulty_R4ilgun\xb176561198351152593\xaf1100001174c75d1\x06\x01\xb32022-02-06 03:04:02\x01\0\0\0\x9d\xcf\x03\x0eS{)\x03G\xe2\t\x07\x0c\t\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x95\xb2210811193631829778\xaeF4ulty_R4ilgun\xb176561198351152593\xaf1100001174c75d1\x06\x02\xb32022-02-06 03:02:20\x01\0\0\0\x9d\xcf\x03\x0eS}\xfct\x97\x16\t\x08\0\x12\x95\xb2210615035914519825\xa5BL4DE\xb176561199083465035\xaf110000142f2a94b\x07\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x01\xb32022-02-06 02:24:18\x01\0\0\0\x9d\xcf\x03\x0eS}\xf3\xeb\x0c\x8a\t\x08\0\x12\x95\xb2210615035914519825\xa5BL4DE\xb176561199083465035\xaf110000142f2a94b\x07\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x02\xb32022-02-06 02:22:34\x01\0\0\0\x9d\xcf\x03\x0eS}\xdb{XM\tc\0\x0e\x95\xb2210611113829735658\xa3Eli\xb176561198449379262\xaf11000011d2747be\t\x95\xb2210612045332227791\xa8R34 I-NO\xb176561198046971684\xaf1100001052b0724\t\x02\xb32022-02-06 02:22:08\x01\0\0\0\x9d\xcf\x03\x0eSy?\xd2\x135\tc\0\x07\x95\xb2210611092701986372\xa3tms\xb176561198223056552\xaf11000010fa9dea8\t\x95\xb2210611184101935607\xb0Shaco Arrombardo\xb176561198019472843\xaf110000103876dcb\t\x02\xb32022-02-06 02:19:53\x01\0\0\0\x9d\xcf\x03\x0eS}\xca\xaeev\tc\0\x0e\x95\xb2210611113829735658\xa3Eli\xb176561198449379262\xaf11000011d2747be\t\x95\xb2210612045332227791\xa8R34 I-NO\xb176561198046971684\xaf1100001052b0724\t\x02\xb32022-02-06 02:19:26\x01\0\0\0\x9d\xcf\x03\x0eSy0\x12\xfd\x84\tc\0\x07\x95\xb2210611092701986372\xa3tms\xb176561198223056552\xaf11000010fa9dea8\t\x95\xb2210611184101935607\xb0Shaco Arrombardo\xb176561198019472843\xaf110000103876dcb\t\x01\xb32022-02-06 02:17:29\x01\0\0\0\x9d\xcf\x03\x0eSy$#\xb0\xfc\tc\0\x07\x95\xb2210611092701986372\xa3tms\xb176561198223056552\xaf11000010fa9dea8\t\x95\xb2210611184101935607\xb0Shaco Arrombardo\xb176561198019472843\xaf110000103876dcb\t\x01\xb32022-02-06 02:15:28\x01\0\0\0\x9d\xcf\x03\x0eS}\xc5\x15\xcf\xf1\t\x08\x12\x12\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x95\xb2210611172901281375\xa4g5h3\xb176561198066767737\xaf110000106591779\x07\x02\xb32022-02-06 02:14:49\x01\0\0\0\x9d\xcf\x03\x0eS}\xb9w\xc3_\t\x08\x12\x12\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x95\xb2210611172901281375\xa4g5h3\xb176561198066767737\xaf110000106591779\x07\x01\xb32022-02-06 02:12:53\x01\0\0\0\x9d\xcf\x03\x0eS}\x95\x1a\x14\xd0\tc\r\0\x95\xb2210611163406897038\xabKidSusSauce\xb176561198796113273\xaf110000131d20579\t\x95\xb2210611113829735658\xa3Eli\xb176561198449379262\xaf11000011d2747be\t\x01\xb32022-02-06 02:10:27\x01\0\0\0\x9d\xcf\x03\x0eS}\xa7$\x04\x91\t\x08\x12\x12\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x95\xb2210611172901281375\xa4g5h3\xb176561198066767737\xaf110000106591779\x07\x01\xb32022-02-06 02:09:46\x01\0\0\0\x9d\xcf\x03\x0eS|x.;\xd4\tc\x01\0\x95\xb2210612195532158554\xa7Nowhere\xb176561198108655731\xaf110000108d84073\t\x95\xb2210611113829735658\xa3Eli\xb176561198449379262\xaf11000011d2747be\t\x02\xb32022-02-06 02:02:47\x01\0\0\0\x9d\xcf\x03\x0eS}re;\xfc\t\x08\x12\x07\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x95\xb2211222194227494329\xacEpicKittyCat\xb176561198040006360\xaf110000104c0bed8\x07\x01\xb32022-02-06 02:01:01\x01\0\0\0\x9d\xcf\x03\x0eS|d\xdd\x9d\x8c\t\x08\x02\x12\x95\xb2211224234141126253\xa6Fakuto\xb176561198387121965\xaf110000119714f2d\x07\x95\xb2210612062056984376\xb0TwitchTV/VRDante\xb176561198067414364\xaf11000010662f55c\x07\x02\xb32022-02-06 01:55:39\x01\0\0\0";
        let mut matches = BTreeSet::new();
        let mut errors = Vec::new();
        parse_response(&mut matches, &mut errors, &RESPONSE);

        assert!(errors.is_empty(), "Got errors: {:#?}", errors);

        expect_test::expect_file!["../test_data/replay_response.txt"].assert_debug_eq(&matches);
    }

    #[test]
    fn test_parse_response_2() {
        // This test used to miss one replay before true messagepack parsing
        const RESPONSE: &[u8] = b"\x92\x98\xad61ff0f60da094\0\xb32022/02/05 23:59:28\xa50.1.0\xa50.0.2\xa50.0.2\xa0\xa0\x94\0\0\n\x9a\x9d\xcf\x03\x0eS}\x9f\x8ds\xbf\t\x08\x0c\x0b\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x95\xb2210818223745601103\xafSamuraiPizzaCat\xb176561199149925226\xaf110000146e8c36a\x07\x02\xb32022-02-06 04:07:59\x01\0\0\0\x9d\xcf\x03\x0eS|v\xbc6N\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:58:19\x01\0\0\0\x9d\xcf\x03\x0eS|lr}\xc1\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:56:46\x01\0\0\0\x9d\xcf\x03\x0eS|du\xac>\t\x08\x11\x0c\x95\xb2210905181006143473\xa8Haratura\xb176561198148293594\xaf11000010b3513da\x07\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x01\xb32022-02-06 03:55:12\x01\x01\0\0\x9d\xcf\x03\x0eSy?\x93\x83\x86\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x01\xb32022-02-06 03:29:31\x01\0\0\0\x9d\xcf\x03\x0eSy/\xfbL\xaa\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x01\xb32022-02-06 03:27:10\x01\0\0\0\x9d\xcf\x03\x0eSy\"\xfc\x1d\x85\t\x06\x04\0\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2211128031436376804\xa9BundleBox\xb176561198103224698\xaf11000010885617a\x05\x02\xb32022-02-06 03:24:52\x01\0\0\0\x9d\xcf\x03\x0eSx\xf9\x8c\xd2\r\t\x06\x04\x12\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2210719021019879063\xa9Sebastard\xb176561198354593280\xaf11000011780f600\x05\x01\xb32022-02-06 03:17:56\x01\0\0\0\x9d\xcf\x03\x0eSx\xedf\x1f\xf4\t\x06\x04\x12\x95\xb2210825010040078270\xacKenoMcsteamo\xb176561198354688358\xaf110000117826966\x05\x95\xb2210719021019879063\xa9Sebastard\xb176561198354593280\xaf11000011780f600\x05\x01\xb32022-02-06 03:15:53\x01\0\0\0\x9d\xcf\x03\x0eS{q&\x8d\x92\t\x07\x05\x0c\x95\xb2220117205818084945\xa8Bugabalu\xb176561198136737187\xaf11000010a84bda3\x05\x95\xb2210611232517053199\xa5limon\xb176561198082398187\xaf1100001074797eb\x06\x02\xb32022-02-06 03:14:30\x01\0\0\0";

        let mut matches = BTreeSet::new();
        let mut errors = Vec::new();
        parse_response(&mut matches, &mut errors, &RESPONSE);

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
