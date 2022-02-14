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
                player_id: "211027113123008384".into(),
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
        floor: replay.floor,
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

#[cfg(test)]
fn from_hex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>()
}
mod messagepack {
    use super::*;

    use serde_crate::{
        de::{Deserializer, Error as _},
        ser::Serializer,
        Deserialize,
    };

    use crate::Character;

    // An integer that we don't know the purpose of in the format. Signed and large to prevent unexpectedly large values from causing errors
    type UnknownInteger = i64;

    pub type ReplayRequest = Request<RequestBody>;

    impl<T> Request<T>
    where
        for<'de> T: Deserialize<'de>,
    {
        #[cfg(test)]
        pub fn from_hex(hex: &str) -> Result<Self> {
            let bytes = from_hex(hex);
            Ok(rmp_serde::decode::from_slice(&bytes)?)
        }
    }

    impl<T> Request<T>
    where
        T: Serialize,
    {
        pub fn to_hex(&self) -> String {
            use std::fmt::Write;

            let mut buf = String::new();
            for b in rmp_serde::encode::to_vec(self).unwrap() {
                write!(buf, "{:02X}", b).unwrap();
            }
            buf
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(crate = "serde_crate")]
    pub struct Request<T> {
        pub header: RequestHeader,
        pub body: T,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(crate = "serde_crate")]
    pub struct RequestHeader {
        // The id of the player making the request (so the server can figure out the follow/rival etc for `PlayerSearch`)
        pub player_id: String,
        pub string2: String,
        pub int1: UnknownInteger,
        pub version: String,
        pub int2: UnknownInteger,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
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
                player_search: PlayerSearch::All,
                min_floor: query.min_floor,
                max_floor: query.max_floor,
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
                prioritize_best_bout: 0,
                int9: 1,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(crate = "serde_crate")]
    pub enum PlayerSearch {
        All,
        Self_,
        Follow,
        Rival,
        Favorite,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(crate = "serde_crate")]
    pub enum RequestWinner {
        Undesignated = -1,
        PlayerOne,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(crate = "serde_crate")]
    pub struct RequestQuery {
        pub int1: UnknownInteger,
        pub player_search: PlayerSearch,
        #[serde(with = "floor")]
        pub min_floor: Floor,
        #[serde(with = "floor")]
        pub max_floor: Floor,
        pub seq: Vec<()>,
        pub char_1: i8,
        pub char_2: i8,
        // 0 for undesignated, 1 for player 1
        pub winner: u8,
        // 0/1 for false/true
        pub prioritize_best_bout: u8,
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
        #[serde(with = "floor")]
        pub floor: Floor,
        pub player1_character: Character,
        pub player2_character: Character,
        pub player1: Player,
        pub player2: Player,
        pub winner: u8,

        #[serde(deserialize_with = "deserialize_date_time")]
        pub date: chrono::DateTime<Utc>,
        pub int7: UnknownInteger,
        pub views: u64,
        pub int8: UnknownInteger,
        pub likes: u64,
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

    #[derive(Debug, Clone, Deserialize, Serialize)]
    #[serde(crate = "serde_crate")]
    pub struct VipRequest {
        pub int1: UnknownInteger,
        pub int2: UnknownInteger,
        pub int3: UnknownInteger,
        pub int4: UnknownInteger,
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

    mod floor {
        use super::*;

        pub(crate) fn deserialize<'de, D>(deserializer: D) -> std::result::Result<Floor, D::Error>
        where
            D: Deserializer<'de>,
        {
            let b = u8::deserialize(deserializer)?;
            Ok(Floor::from_u8(b).map_err(D::Error::custom)?)
        }

        pub(crate) fn serialize<S>(
            value: &Floor,
            serializer: S,
        ) -> std::result::Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            value.to_u8().serialize(serializer)
        }
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
    fn test_parse_response_3() {
        // This test used to miss one replay before true messagepack parsing
        const RESPONSE: &[u8] = b"\x92\x98\xad61ffa1560e387\0\xb32022/02/06 10:22:14\xa50.1.0\xa50.0.2\xa50.0.2\xa0\xa0\x94\0\x04\n\x9a\x9d\xcf\x03\x0e\n\xb0\x95(\xcd2\x07c\x06\x07\x95\xb2210611073056107537\xa3Mar\xb176561197993198569\xaf110000101f683e9\t\x95\xb2210611121603560347\xadLuna Goodgirl\xb176561197977446342\xaf1100001010627c6\t\x01\xb32022-01-25 18:53:19\x01\x01\x01\x01\x9d\xcf\x03\r\xfb5{F6\"\x07c\x06\x07\x95\xb2210611073056107537\xa3Mar\xb176561197993198569\xaf110000101f683e9\t\x95\xb2210611162113864298\xa8Lizardos\xb176561197994492361\xaf1100001020a41c9\t\x01\xb32022-01-08 16:39:30\x01\x03\x01\x01\x9d\xcf\x02\xed\xbb\xb9\x7f\xdd?!\x06c\x06\x05\x95\xb2210611073056107537\xa3Mar\xb176561197993198569\xaf110000101f683e9\t\x95\xb2210611095248078392\xa9MOMO MODY\xb176561198156904572\xaf11000010bb8787c\t\x02\xb32021-10-31 16:29:42\x01\x03\x02\0\x9d\xcf\x02\xed\xa2D\x07\x98m\x1a\x05\n\x06\x0b\x95\xb2210611073056107537\xa3Mar\xb176561197993198569\xaf110000101f683e9\t\x95\xb2210611083023337322\xadPunishedVenom\xb176561198043848438\xaf110000104fb5ef6\t\x02\xb32021-10-03 17:06:24\x01\0\x02\0\x9d\xcf\x02\xec\xef\x05\xe7\xe6\xf1\x88\x04\n\x08\x07\x95\xb2210611073056107537\xa3Mar\xb176561197993198569\xaf110000101f683e9\t\x95\xb2210611072758921052\xaageorgekupo\xb176561198054369781\xaf1100001059be9f5\t\x02\xb32021-08-06 09:12:22\x01\0\x02\0\x9d\xcf\x02\xec\xed6\xf1\xea8\xbe\x04\n\x08\x0b\x95\xb2210611073056107537\xa3Mar\xb176561197993198569\xaf110000101f683e9\x08\x95\xb2210618173410867109\xabDominimator\xb176561197994451661\xaf11000010209a2cd\t\x02\xb32021-08-04 10:28:20\x01\0\x02\0\x9d\xcf\x02\xecG\xc9\xde*\x8e\xd6\x03\x07\x08\x01\x95\xb2210611073056107537\xa3Mar\xb176561197993198569\xaf110000101f683e9\x06\x95\xb2210614203011010057\xa5Hydro\xb176561198077327061\xaf110000106fa36d5\x06\x02\xb32021-06-22 21:49:19\x01\0\x02\0\x9d\xcf\x03\x0eSo\xd3qzH\tc\r\x06\x95\xb2210611114424649707\xa9Pistachio\xb176561198074756096\xaf110000106d2fc00\t\x95\xb2210611073056107537\xa3Mar\xb176561197993198569\xaf110000101f683e9\t\x02\xb32022-02-05 17:15:39\x01\0\0\0\x9d\xcf\x03\x0eSo\xbc\x9cz\x82\tc\x02\x06\x95\xb2210611151221285918\xa7Rikkumi\xb176561198117246557\xaf1100001095b565d\t\x95\xb2210611073056107537\xa3Mar\xb176561197993198569\xaf110000101f683e9\t\x02\xb32022-02-05 17:11:56\x01\0\0\0\x9d\xcf\x03\x0eSo\xae.\x18\xcc\tc\x02\x06\x95\xb2210611151221285918\xa7Rikkumi\xb176561198117246557\xaf1100001095b565d\t\x95\xb2210611073056107537\xa3Mar\xb176561197993198569\xaf110000101f683e9\t\x02\xb32022-02-05 17:09:14\x01\0\0\0";

        let result = rmp_serde::decode::from_slice::<messagepack::ReplayResponse>(&RESPONSE);

        expect_test::expect_file!["../test_data/replay_response_3.txt"].assert_debug_eq(&result);
    }

    #[test]
    fn test_parse_response_4() {
        // This test used to miss one replay before true messagepack parsing
        const RESPONSE: &[u8] = b"\x92\x98\xad61ffa6c3dce48\x00\xb32022/02/06 10:45:23\xa50.1.0\xa50.0.2\xa50.0.2\xa0\xa0\x94\x00\x00\x14\xdc\x00\x14\x9d\xcf\x03\x0eTH\xb4\x9fm\xae\tc\x0c\x00\x95\xb2210612125643406306\xa6\xe3\x81\xab\xe3\x81\x97\xb176561198128581292\xaf11000010a084aac\t\x95\xb2210812201532300023\xa4Aya_\xb176561198082485936\xaf11000010748eeb0\t\x01\xb32022-02-06 10:30:35\x01\x00\x04\x00\x9d\xcf\x03\x0eTH\xb4\xe79|\t\x07\x0e\r\x95\xb2210615052252624822\xa7kenwood\xb176561197966537714\xaf1100001005fb3f2\x06\x95\xb2210611154646317449\xadSacral Choppa\xb176561199006810534\xaf11000013e6101a6\x06\x01\xb32022-02-06 10:30:34\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4\xb6\\\xe7\t\x08\x12\x02\x95\xb2210612021027770109\xafSEAFOOD_TEACHER\xb176561198113434879\xaf110000109212cff\x07\x95\xb2211207080045848646\xa4Snao\xb176561199222646653\xaf11000014b3e677d\x07\x02\xb32022-02-06 10:30:33\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4\xb5*e\tc\x07\x00\x95\xb2210613140711574755\xac\xed\x95\xa0\xeb\x9d\xbc\xed\x94\xbc\xeb\x87\xa8\xb176561198864345829\xaf110000135e32ae5\t\x95\xb2210611143729214686\xa3kim\xb176561198854003264\xaf110000135455a40\t\x01\xb32022-02-06 10:30:33\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4\xb0:E\tc\x0c\t\x95\xb2210611072136083266\xafKiiwiFrankenCop\xb176561198895319862\xaf110000137bbcb36\t\x95\xb2210611182927774405\xa9Mr. Quick\xb176561198069518514\xaf1100001068310b2\t\x02\xb32022-02-06 10:30:32\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4\xaf[Z\t\x06\x0f\x12\x95\xb2220121231856937297\xaaThoraxe237\xb176561198052581773\xaf11000010580a18d\x05\x95\xb2210613005107516525\xa8Keshabro\xb176561198027398330\xaf110000104005cba\x04\x01\xb32022-02-06 10:30:32\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4\xb0\xbd\x1f\t\x08\x0c\n\x95\xb2210912022814996615\xa7highlow\xb176561199205533603\xaf11000014a3947a3\x07\x95\xb2210611085648495430\xa8nametake\xb176561199149370171\xaf110000146e04b3b\x07\x01\xb32022-02-06 10:30:31\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4\xa8\x8c\x06\t\t\x00\x05\x95\xb2210611073057022504\xa8AlphaMJB\xb176561198305607584\xaf110000114957fa0\x08\x95\xb2220127151856058147\xaf\xe3\x82\xaf\xe3\x83\xa9\xe3\x83\x83\xe3\x82\xb7\xe3\x83\xa5\xb176561198165187796\xaf11000010c36dcd4\x08\x02\xb32022-02-06 10:30:29\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4\x807d\t\n\r\x12\x95\xb2210611084139551457\xaaDivin#1214\xb176561198077941403\xaf11000010703969b\t\x95\xb2210611101724829815\xaeRez:Gilgystera\xb176561198132106791\xaf11000010a3e1627\t\x01\xb32022-02-06 10:30:29\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4d\r\xce\t\n\x07\x12\x95\xb2211222225908577640\xa7Cotezzo\xb176561198421841583\xaf11000011b8316af\x08\x95\xb2210611071849576512\xa7Taiga2k\xb176561198040834092\xaf110000104cd602c\x08\x01\xb32022-02-06 10:30:27\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb3\xbeV\xbd\t\n\x0c\r\x95\xb2210721083447239477\xac\xed\x96\x89\xeb\xb3\xb5\xed\x9a\x8c\xeb\xa1\x9c\xb176561198058727476\xaf110000105de6834\t\x95\xb2210828085855460099\xb8\xe4\xbf\xa1\xe5\xb7\x9e\xe7\x84\xa1\xe6\x95\xb5\xe3\x81\xae\xe6\xa1\x83\xe5\xa4\xaa\xe9\x83\x8e\xb176561198138785803\xaf11000010aa4000b\x08\x02\xb32022-02-06 10:30:26\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb47\x1f+\t\x08\x02\x00\x95\xb2210612121526544046\xa6\xe3\x81\xb5\xe3\x82\x8f\xb176561199174118419\xaf11000014859ec13\x07\x95\xb2210611094539865120\xa3lan\xb176561198317011665\xaf1100001154382d1\x07\x01\xb32022-02-06 10:30:25\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4\x1b\x8ds\t\t\x0b\x00\x95\xb2210617234253473467\xaeballsack_penis\xb176561198055995469\xaf110000105b4b84d\x08\x95\xb2210611071427578001\xa6Xsaber\xb176561198101112765\xaf1100001086527bd\x08\x02\xb32022-02-06 10:30:22\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4\x0e(\x04\tc\x12\x00\x95\xb2210811113031312233\xac\xe3\x81\xbe\xe3\x81\x9f\xe3\x82\x8f\xe3\x82\x8a\xb176561198196931129\xaf11000010e1b3a39\t\x95\xb2210811153641989054\xb2\xe3\x81\x99\xe3\x81\xb4\xe3\x81\x8b\xe3\x81\xa1\xe3\x82\x83\xe3\x82\x93\xb176561199123357584\xaf110000145535f90\t\x02\xb32022-02-06 10:30:22\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4\x0e\\\xe6\tc\x00\x06\x95\xb2210611145942951029\xabPlaceholder\xb176561198123582712\xaf110000109bc04f8\t\x95\xb2210613043239093829\xadMouljaveel-PC\xb176561198105342214\xaf110000108a5b106\t\x02\xb32022-02-06 10:30:21\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb4\x04\xfb7\t\n\x0e\x0b\x95\xb2210729195702260121\xaemystery cruise\xb176561197980107402\xaf1100001012ec28a\t\x95\xb2211231003426173119\xaaWilling555\xb176561198158500699\xaf11000010bd0d35b\t\x01\xb32022-02-06 10:30:20\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb2\xd2\xdb\x17\t\n\x01\x12\x95\xb2210611080232191761\xaaKOIBITO\xef\xbc\x81\xb176561198159124250\xaf11000010bda571a\t\x95\xb2210708134321624142\xa5loser\xb176561198207840299\xaf11000010ec1b02b\t\x01\xb32022-02-06 10:30:20\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb3\xc3y\xcc\t\n\x12\x0c\x95\xb2220117164656998999\xa4syan\xb176561199204162898\xaf11000014a245d52\t\x95\xb2210618050710408587\xb8\xe5\x90\x89\xe7\x94\xb0\xe3\x83\x92\xe3\x83\xad\xe3\x83\x95\xe3\x83\x9f\xe3\x81\xae\xe5\xa5\xb3\xb176561198395837298\xaf110000119f64b72\t\x02\xb32022-02-06 10:30:18\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xb3\xbb^K\t\x07\r\x01\x95\xb2210612065256836370\xa9Aquascape\xb176561198108384313\xaf110000108d41c39\x06\x95\xb2211207004536721762\xacBaldilocksTM\xb176561199057152272\xaf110000141612910\x06\x01\xb32022-02-06 10:30:17\x01\x00\x00\x00\x9d\xcf\x03\x0eTH\xbb.B\xa7\t\n\x12\x01\x95\xb2210611155821768595\xadJ A I G E R E\xb176561198835237053\xaf1100001342700bd\t\x95\xb2210611133136888481\xaf\xe3\x81\x95\xe3\x82\x84\xe3\x81\x8b\xe3\x81\x95\xe3\x82\x93\xb176561198006011479\xaf110000102ba0657\t\x02\xb32022-02-06 10:30:16\x01\x00\x00\x00";

        let mut de = rmp_serde::decode::Deserializer::from_read_ref(RESPONSE);
        let result = serde_path_to_error::deserialize::<_, messagepack::ReplayResponse>(&mut de)
            .map_err(|err| err.to_string());

        expect_test::expect_file!["../test_data/replay_response_4.txt"].assert_debug_eq(&result);
    }

    #[test]
    fn test_query() {
        use messagepack::*;

        let query = ReplayRequest {
            header: RequestHeader {
                player_id: "211027113123008384".into(),
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
                    player_search: PlayerSearch::All,
                    min_floor: Floor::F1,
                    max_floor: Floor::Celestial,
                    seq: vec![],
                    char_1: -1,
                    char_2: -1,
                    winner: 0,
                    prioritize_best_bout: 0,
                    int9: 1,
                },
            },
        };

        expect_test::expect![[r#"9295B2323131303237313133313233303038333834AD3631613565643466343631633202A5302E312E30039401007F9AFFA3416C6C016390FFFF000001"#]].assert_eq(&query.to_hex())
    }

    #[test]
    fn decode_request() {
        let request = messagepack::ReplayRequest::from_hex("9295b2323130363131303733303536313037353337ad3631666639366131653762353902a5302e312e30039401000a9aff02016390ffff000101").unwrap();
        expect_test::expect![[r#"
            ReplayRequest {
                header: RequestHeader {
                    player_id: "210611073056107537",
                    string2: "61ff96a1e7b59",
                    int1: 2,
                    version: "0.1.0",
                    int2: 3,
                },
                body: RequestBody {
                    int1: 1,
                    index: 0,
                    replays_per_page: 10,
                    query: RequestQuery {
                        int1: -1,
                        player_search: Follow,
                        min_floor: F1,
                        max_floor: Celestial,
                        seq: [],
                        char_1: -1,
                        char_2: -1,
                        winner: 0,
                        prioritize_best_bout: 1,
                        int9: 1,
                    },
                },
            }
        "#]]
        .assert_debug_eq(&request);
    }

    #[test]
    fn decode_vip_ranking_request() {
        let request = messagepack::Request::<messagepack::VipRequest>::from_hex("9295b2323130363131303733303536313037353337ad3632306132363930623165653102a5302e312e3003940000ff00").unwrap();

        expect_test::expect![[r#"
            Request {
                header: RequestHeader {
                    player_id: "210611073056107537",
                    string2: "620a2690b1ee1",
                    int1: 2,
                    version: "0.1.0",
                    int2: 3,
                },
                body: VipRequest {
                    int1: 0,
                    int2: 0,
                    int3: -1,
                    int4: 0,
                },
            }
        "#]]
        .assert_debug_eq(&request);
    }
}
