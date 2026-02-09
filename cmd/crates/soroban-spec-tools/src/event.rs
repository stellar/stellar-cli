use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;
use stellar_xdr::curr::{
    ScSpecEventDataFormat, ScSpecEventParamLocationV0, ScSpecEventParamV0, ScSpecEventV0, ScSymbol,
    ScVal,
};

use crate::{Error, Spec};

/// Decoded event with named parameters
#[derive(Debug, Clone, Serialize)]
pub struct DecodedEvent {
    pub contract_id: String,
    /// The event name from the contract spec (e.g., "Transfer", "Approve")
    pub event_name: String,
    /// The prefix topics that identify this event (e.g., `["transfer"]`)
    pub prefix_topics: Vec<String>,
    pub params: IndexMap<String, Value>,
}

/// Errors that can occur during event decoding
#[derive(thiserror::Error, Debug)]
pub enum EventDecodeError {
    #[error("No matching event spec found")]
    NoMatchingSpec,
    #[error("Topic count mismatch: expected at least {expected}, got {actual}")]
    TopicCountMismatch { expected: usize, actual: usize },
    #[error("Data parameter count mismatch: expected {expected}, got {actual}")]
    DataParamCountMismatch { expected: usize, actual: usize },
    #[error("Failed to decode parameter '{name}': {source}")]
    ParamDecodeError { name: String, source: Error },
    #[error("Invalid topic format: expected symbol")]
    InvalidTopicFormat,
    #[error("Invalid data format for event")]
    InvalidDataFormat,
    #[error("Spec error: {0}")]
    SpecError(#[from] Error),
}

impl Spec {
    /// Match event topics to find the corresponding spec
    ///
    /// Returns the matching event spec if the prefix topics match, otherwise None.
    pub fn match_event_to_spec<'a>(&'a self, topics: &[ScVal]) -> Option<&'a ScSpecEventV0> {
        self.find_events()
            .ok()?
            .find(|event| matches_prefix_topics(&event.prefix_topics, topics))
    }

    /// Decode event using spec, producing named parameters
    ///
    /// # Errors
    ///
    /// Returns an error if the event cannot be decoded
    pub fn decode_event(
        &self,
        contract_id: &str,
        topics: &[ScVal],
        data: &ScVal,
    ) -> Result<DecodedEvent, EventDecodeError> {
        let event_spec = self
            .match_event_to_spec(topics)
            .ok_or(EventDecodeError::NoMatchingSpec)?;

        decode_event_with_spec(self, contract_id, topics, data, event_spec)
    }
}

/// Check if the prefix topics match the first N event topics
fn matches_prefix_topics(prefix_topics: &[ScSymbol], topics: &[ScVal]) -> bool {
    if prefix_topics.is_empty() {
        return true;
    }

    // Need at least as many topics as prefix topics
    if topics.len() < prefix_topics.len() {
        return false;
    }

    // Check each prefix topic matches the corresponding event topic.
    prefix_topics
        .iter()
        .zip(topics.iter())
        .all(|(prefix, topic)| match topic {
            ScVal::Symbol(topic_sym) => prefix.as_vec() == topic_sym.as_vec(),
            ScVal::String(topic_str) => prefix.as_vec() == topic_str.as_vec(),
            _ => false,
        })
}

/// Decode an event using the provided spec
fn decode_event_with_spec(
    spec: &Spec,
    contract_id: &str,
    topics: &[ScVal],
    data: &ScVal,
    event_spec: &ScSpecEventV0,
) -> Result<DecodedEvent, EventDecodeError> {
    let event_name = event_spec.name.to_utf8_string_lossy();
    let mut params = IndexMap::new();

    // Separate params by location
    let (topic_params, data_params): (Vec<_>, Vec<_>) = event_spec
        .params
        .iter()
        .partition(|p| p.location == ScSpecEventParamLocationV0::TopicList);

    // Skip past prefix topics to get to the parameter topics
    let topic_offset = event_spec.prefix_topics.len();

    // Extract topic parameters
    extract_topic_params(spec, topics, &topic_params, topic_offset, &mut params)?;

    // Extract data parameters based on data_format
    extract_data_params(
        spec,
        data,
        &data_params,
        event_spec.data_format,
        &mut params,
    )?;

    let prefix_topics = event_spec
        .prefix_topics
        .iter()
        .map(|t| t.to_utf8_string_lossy())
        .collect();

    Ok(DecodedEvent {
        contract_id: contract_id.to_string(),
        event_name,
        prefix_topics,
        params,
    })
}

/// Extract parameters from topics
fn extract_topic_params(
    spec: &Spec,
    topics: &[ScVal],
    topic_params: &[&ScSpecEventParamV0],
    topic_offset: usize,
    params: &mut IndexMap<String, Value>,
) -> Result<(), EventDecodeError> {
    for (i, param) in topic_params.iter().enumerate() {
        let topic_idx = topic_offset + i;
        if topic_idx >= topics.len() {
            // Topic count doesn't match spec - likely a spec mismatch
            return Err(EventDecodeError::TopicCountMismatch {
                expected: topic_offset + topic_params.len(),
                actual: topics.len(),
            });
        }

        let param_name = param.name.to_utf8_string_lossy();
        let topic_value = &topics[topic_idx];

        let json_value = spec.xdr_to_json(topic_value, &param.type_).map_err(|e| {
            EventDecodeError::ParamDecodeError {
                name: param_name.clone(),
                source: e,
            }
        })?;

        params.insert(param_name, json_value);
    }

    Ok(())
}

/// Extract parameters from event data based on the data format
fn extract_data_params(
    spec: &Spec,
    data: &ScVal,
    data_params: &[&ScSpecEventParamV0],
    data_format: ScSpecEventDataFormat,
    params: &mut IndexMap<String, Value>,
) -> Result<(), EventDecodeError> {
    if data_params.is_empty() {
        return Ok(());
    }

    match data_format {
        ScSpecEventDataFormat::SingleValue => {
            // Single value - should have exactly one data param
            if let Some(param) = data_params.first() {
                let param_name = param.name.to_utf8_string_lossy();
                let json_value = spec.xdr_to_json(data, &param.type_).map_err(|e| {
                    EventDecodeError::ParamDecodeError {
                        name: param_name.clone(),
                        source: e,
                    }
                })?;
                params.insert(param_name, json_value);
            }
        }
        ScSpecEventDataFormat::Vec => {
            // Vec format - data should be a Vec with elements matching params
            if let ScVal::Vec(Some(vec)) = data {
                for (i, param) in data_params.iter().enumerate() {
                    if i >= vec.len() {
                        break;
                    }
                    let param_name = param.name.to_utf8_string_lossy();
                    let json_value = spec.xdr_to_json(&vec[i], &param.type_).map_err(|e| {
                        EventDecodeError::ParamDecodeError {
                            name: param_name.clone(),
                            source: e,
                        }
                    })?;
                    params.insert(param_name, json_value);
                }
            } else {
                return Err(EventDecodeError::InvalidDataFormat);
            }
        }
        ScSpecEventDataFormat::Map => {
            // Map format - data should be a Map with keys matching param names
            if let ScVal::Map(Some(map)) = data {
                for param in data_params {
                    let param_name = param.name.to_utf8_string_lossy();
                    // Find the map entry with matching key
                    if let Some(entry) = map.iter().find(|entry| {
                        if let ScVal::Symbol(sym) = &entry.key {
                            sym.to_utf8_string_lossy() == param_name
                        } else {
                            false
                        }
                    }) {
                        let json_value =
                            spec.xdr_to_json(&entry.val, &param.type_).map_err(|e| {
                                EventDecodeError::ParamDecodeError {
                                    name: param_name.clone(),
                                    source: e,
                                }
                            })?;
                        params.insert(param_name, json_value);
                    }
                }
            } else {
                return Err(EventDecodeError::InvalidDataFormat);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use stellar_xdr::curr::{
        Int128Parts, ScMap, ScMapEntry, ScSpecEntry, ScSpecTypeDef, ScString, ScVec, StringM, VecM,
    };

    fn make_symbol(s: &str) -> ScSymbol {
        ScSymbol(s.try_into().unwrap())
    }

    fn make_sc_symbol(s: &str) -> ScVal {
        ScVal::Symbol(make_symbol(s))
    }

    fn make_i128(val: i128) -> ScVal {
        let bytes = val.to_be_bytes();
        let (hi, lo) = bytes.split_at(8);
        ScVal::I128(Int128Parts {
            hi: i64::from_be_bytes(hi.try_into().unwrap()),
            lo: u64::from_be_bytes(lo.try_into().unwrap()),
        })
    }

    fn make_event_param(
        name: &str,
        type_: ScSpecTypeDef,
        location: ScSpecEventParamLocationV0,
    ) -> ScSpecEventParamV0 {
        ScSpecEventParamV0 {
            doc: StringM::default(),
            name: name.try_into().unwrap(),
            type_,
            location,
        }
    }

    fn make_event_spec(
        name: &str,
        prefix_topics: Vec<&str>,
        params: Vec<ScSpecEventParamV0>,
        data_format: ScSpecEventDataFormat,
    ) -> ScSpecEventV0 {
        ScSpecEventV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: make_symbol(name),
            prefix_topics: prefix_topics
                .into_iter()
                .map(make_symbol)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            params: params.try_into().unwrap(),
            data_format,
        }
    }

    fn make_spec_with_events(events: Vec<ScSpecEventV0>) -> Spec {
        let entries: Vec<ScSpecEntry> = events.into_iter().map(ScSpecEntry::EventV0).collect();
        Spec::new(&entries)
    }

    #[test]
    fn test_matches_prefix_topics_empty_prefix() {
        let prefix: VecM<ScSymbol, 2> = VecM::default();
        let topics = vec![make_sc_symbol("transfer")];
        assert!(matches_prefix_topics(&prefix, &topics));
    }

    #[test]
    fn test_matches_prefix_topics_single_match() {
        let prefix: VecM<ScSymbol, 2> = vec![make_symbol("transfer")].try_into().unwrap();
        let topics = vec![make_sc_symbol("transfer"), make_sc_symbol("from")];
        assert!(matches_prefix_topics(&prefix, &topics));
    }

    #[test]
    fn test_matches_prefix_topics_two_prefixes_match() {
        let prefix: VecM<ScSymbol, 2> = vec![make_symbol("token"), make_symbol("transfer")]
            .try_into()
            .unwrap();
        let topics = vec![
            make_sc_symbol("token"),
            make_sc_symbol("transfer"),
            make_sc_symbol("from"),
        ];
        assert!(matches_prefix_topics(&prefix, &topics));
    }

    #[test]
    fn test_matches_prefix_topics_mismatch() {
        let prefix: VecM<ScSymbol, 2> = vec![make_symbol("approve")].try_into().unwrap();
        let topics = vec![make_sc_symbol("transfer")];
        assert!(!matches_prefix_topics(&prefix, &topics));
    }

    #[test]
    fn test_matches_prefix_topics_insufficient_topics() {
        let prefix: VecM<ScSymbol, 2> =
            vec![make_symbol("a"), make_symbol("b")].try_into().unwrap();
        let topics = vec![make_sc_symbol("a")];
        assert!(!matches_prefix_topics(&prefix, &topics));
    }

    #[test]
    fn test_matches_prefix_topics_non_symbol_topic() {
        let prefix: VecM<ScSymbol, 2> = vec![make_symbol("transfer")].try_into().unwrap();
        let topics = vec![ScVal::U32(123)]; // Not a symbol or string
        assert!(!matches_prefix_topics(&prefix, &topics));
    }

    #[test]
    fn test_matches_prefix_topics_string_topic() {
        // Some early contracts use String instead of Symbol
        let prefix: VecM<ScSymbol, 2> = vec![make_symbol("transfer")].try_into().unwrap();
        let s: StringM = "transfer".try_into().unwrap();
        let topics = vec![ScVal::String(ScString(s))];
        assert!(matches_prefix_topics(&prefix, &topics));
    }

    #[test]
    fn test_match_event_to_spec_with_prefix() {
        let event = make_event_spec(
            "transfer",
            vec!["token"],
            vec![],
            ScSpecEventDataFormat::SingleValue,
        );
        let spec = make_spec_with_events(vec![event]);

        let topics = vec![make_sc_symbol("token"), make_sc_symbol("transfer")];
        let matched = spec.match_event_to_spec(&topics);

        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name.to_utf8_string_lossy(), "transfer");
    }

    #[test]
    fn test_match_event_to_spec_not_found() {
        let event = make_event_spec(
            "transfer",
            vec!["transfer"],
            vec![],
            ScSpecEventDataFormat::SingleValue,
        );
        let spec = make_spec_with_events(vec![event]);

        let topics = vec![make_sc_symbol("approve")];
        let matched = spec.match_event_to_spec(&topics);

        assert!(matched.is_none());
    }

    #[test]
    fn test_match_event_to_spec_multiple_events() {
        let transfer_event = make_event_spec(
            "transfer",
            vec!["transfer"],
            vec![],
            ScSpecEventDataFormat::SingleValue,
        );
        let approve_event = make_event_spec(
            "approve",
            vec!["approve"],
            vec![],
            ScSpecEventDataFormat::SingleValue,
        );
        let spec = make_spec_with_events(vec![transfer_event, approve_event]);

        let topics = vec![make_sc_symbol("approve")];
        let matched = spec.match_event_to_spec(&topics);

        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name.to_utf8_string_lossy(), "approve");
    }

    #[test]
    fn test_decode_event_single_value_data() {
        let event = make_event_spec(
            "transfer",
            vec!["transfer"],
            vec![make_event_param(
                "amount",
                ScSpecTypeDef::I128,
                ScSpecEventParamLocationV0::Data,
            )],
            ScSpecEventDataFormat::SingleValue,
        );
        let spec = make_spec_with_events(vec![event]);

        let topics = vec![make_sc_symbol("transfer")];
        let data = make_i128(1000);

        let decoded = spec.decode_event("CABC123", &topics, &data).unwrap();

        assert_eq!(decoded.contract_id, "CABC123");
        assert_eq!(decoded.event_name, "transfer");
        assert_eq!(decoded.params.get("amount"), Some(&json!("1000")));
    }

    #[test]
    fn test_decode_event_with_topic_params() {
        // Event with prefix_topics identifying the event, plus topic params
        let event = make_event_spec(
            "transfer",
            vec!["transfer"], // prefix_topics identifies this event
            vec![
                make_event_param(
                    "from",
                    ScSpecTypeDef::Symbol,
                    ScSpecEventParamLocationV0::TopicList,
                ),
                make_event_param(
                    "amount",
                    ScSpecTypeDef::I128,
                    ScSpecEventParamLocationV0::Data,
                ),
            ],
            ScSpecEventDataFormat::SingleValue,
        );
        let spec = make_spec_with_events(vec![event]);

        // topics[0] = "transfer" (prefix), topics[1] = "alice" (from param)
        let topics = vec![make_sc_symbol("transfer"), make_sc_symbol("alice")];
        let data = make_i128(500);

        let decoded = spec.decode_event("CONTRACT", &topics, &data).unwrap();

        assert_eq!(decoded.event_name, "transfer");
        assert_eq!(decoded.params.get("from"), Some(&json!("alice")));
        assert_eq!(decoded.params.get("amount"), Some(&json!("500")));
    }

    #[test]
    fn test_decode_event_vec_data_format() {
        let event = make_event_spec(
            "multi",
            vec!["multi"],
            vec![
                make_event_param("a", ScSpecTypeDef::I128, ScSpecEventParamLocationV0::Data),
                make_event_param("b", ScSpecTypeDef::I128, ScSpecEventParamLocationV0::Data),
            ],
            ScSpecEventDataFormat::Vec,
        );
        let spec = make_spec_with_events(vec![event]);

        let topics = vec![make_sc_symbol("multi")];
        let data = ScVal::Vec(Some(ScVec(
            vec![make_i128(100), make_i128(200)].try_into().unwrap(),
        )));

        let decoded = spec.decode_event("CONTRACT", &topics, &data).unwrap();

        assert_eq!(decoded.params.get("a"), Some(&json!("100")));
        assert_eq!(decoded.params.get("b"), Some(&json!("200")));
    }

    #[test]
    fn test_decode_event_map_data_format() {
        let event = make_event_spec(
            "info",
            vec!["info"],
            vec![
                make_event_param(
                    "name",
                    ScSpecTypeDef::Symbol,
                    ScSpecEventParamLocationV0::Data,
                ),
                make_event_param(
                    "value",
                    ScSpecTypeDef::I128,
                    ScSpecEventParamLocationV0::Data,
                ),
            ],
            ScSpecEventDataFormat::Map,
        );
        let spec = make_spec_with_events(vec![event]);

        let topics = vec![make_sc_symbol("info")];
        let data = ScVal::Map(Some(ScMap(
            vec![
                ScMapEntry {
                    key: make_sc_symbol("name"),
                    val: make_sc_symbol("test"),
                },
                ScMapEntry {
                    key: make_sc_symbol("value"),
                    val: make_i128(42),
                },
            ]
            .try_into()
            .unwrap(),
        )));

        let decoded = spec.decode_event("CONTRACT", &topics, &data).unwrap();

        assert_eq!(decoded.params.get("name"), Some(&json!("test")));
        assert_eq!(decoded.params.get("value"), Some(&json!("42")));
    }

    #[test]
    fn test_decode_event_no_matching_spec() {
        let event = make_event_spec(
            "transfer",
            vec!["transfer"],
            vec![],
            ScSpecEventDataFormat::SingleValue,
        );
        let spec = make_spec_with_events(vec![event]);

        let topics = vec![make_sc_symbol("unknown")];
        let data = ScVal::Void;

        let result = spec.decode_event("CONTRACT", &topics, &data);
        assert!(matches!(result, Err(EventDecodeError::NoMatchingSpec)));
    }

    #[test]
    fn test_decode_event_invalid_vec_data_format() {
        let event = make_event_spec(
            "test",
            vec!["test"],
            vec![make_event_param(
                "a",
                ScSpecTypeDef::I128,
                ScSpecEventParamLocationV0::Data,
            )],
            ScSpecEventDataFormat::Vec,
        );
        let spec = make_spec_with_events(vec![event]);

        let topics = vec![make_sc_symbol("test")];
        let data = make_i128(100); // Should be Vec, not single value

        let result = spec.decode_event("CONTRACT", &topics, &data);
        assert!(matches!(result, Err(EventDecodeError::InvalidDataFormat)));
    }

    #[test]
    fn test_decode_event_invalid_map_data_format() {
        let event = make_event_spec(
            "test",
            vec!["test"],
            vec![make_event_param(
                "a",
                ScSpecTypeDef::I128,
                ScSpecEventParamLocationV0::Data,
            )],
            ScSpecEventDataFormat::Map,
        );
        let spec = make_spec_with_events(vec![event]);

        let topics = vec![make_sc_symbol("test")];
        let data = make_i128(100); // Should be Map, not single value

        let result = spec.decode_event("CONTRACT", &topics, &data);
        assert!(matches!(result, Err(EventDecodeError::InvalidDataFormat)));
    }

    #[test]
    fn test_decode_event_empty_spec() {
        let spec = make_spec_with_events(vec![]);

        let topics = vec![make_sc_symbol("transfer")];
        let data = ScVal::Void;

        let result = spec.decode_event("CONTRACT", &topics, &data);
        assert!(matches!(result, Err(EventDecodeError::NoMatchingSpec)));
    }

    #[test]
    fn test_decode_event_preserves_param_order() {
        let event = make_event_spec(
            "ordered",
            vec!["ordered"],
            vec![
                make_event_param(
                    "first",
                    ScSpecTypeDef::I128,
                    ScSpecEventParamLocationV0::Data,
                ),
                make_event_param(
                    "second",
                    ScSpecTypeDef::I128,
                    ScSpecEventParamLocationV0::Data,
                ),
                make_event_param(
                    "third",
                    ScSpecTypeDef::I128,
                    ScSpecEventParamLocationV0::Data,
                ),
            ],
            ScSpecEventDataFormat::Vec,
        );
        let spec = make_spec_with_events(vec![event]);

        let topics = vec![make_sc_symbol("ordered")];
        let data = ScVal::Vec(Some(ScVec(
            vec![make_i128(1), make_i128(2), make_i128(3)]
                .try_into()
                .unwrap(),
        )));

        let decoded = spec.decode_event("CONTRACT", &topics, &data).unwrap();

        // Verify order is preserved using IndexMap
        let keys: Vec<_> = decoded.params.keys().collect();
        assert_eq!(keys, vec!["first", "second", "third"]);
    }

    #[test]
    fn test_decode_event_no_data_params() {
        // Event with only topic params, no data params
        let event = make_event_spec(
            "simple",
            vec!["simple"], // prefix_topics identifies this event
            vec![make_event_param(
                "who",
                ScSpecTypeDef::Symbol,
                ScSpecEventParamLocationV0::TopicList,
            )],
            ScSpecEventDataFormat::SingleValue,
        );
        let spec = make_spec_with_events(vec![event]);

        // topics[0] = "simple" (prefix), topics[1] = "alice" (who param)
        let topics = vec![make_sc_symbol("simple"), make_sc_symbol("alice")];
        let data = ScVal::Void;

        let decoded = spec.decode_event("CONTRACT", &topics, &data).unwrap();

        assert_eq!(decoded.params.len(), 1);
        assert_eq!(decoded.params.get("who"), Some(&json!("alice")));
    }

    #[test]
    fn test_decoded_event_json_serialization() {
        let mut params = IndexMap::new();
        params.insert("from".to_string(), json!("alice"));
        params.insert("to".to_string(), json!("bob"));
        params.insert("amount".to_string(), json!("1000"));

        let decoded = DecodedEvent {
            contract_id: "CABC123".to_string(),
            event_name: "transfer".to_string(),
            prefix_topics: vec!["transfer".to_string()],
            params,
        };

        let json_str = serde_json::to_string(&decoded).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["contract_id"], "CABC123");
        assert_eq!(parsed["event_name"], "transfer");
        assert_eq!(parsed["prefix_topics"][0], "transfer");
        assert_eq!(parsed["params"]["from"], "alice");
        assert_eq!(parsed["params"]["to"], "bob");
        assert_eq!(parsed["params"]["amount"], "1000");
    }
}
