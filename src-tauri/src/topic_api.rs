use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

pub(crate) struct TopicApiPaths {
    base_url: String,
}

impl TopicApiPaths {
    pub(crate) fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub(crate) fn topics(&self) -> String {
        format!("{}/api/v1/topics", self.base_url)
    }

    pub(crate) fn messages(&self, topic: &str) -> String {
        format!("{}/{}/messages", self.topics(), encode_segment(topic))
    }

    pub(crate) fn partition_messages(&self, topic: &str, partition: usize, limit: usize) -> String {
        format!(
            "{}/{}/partitions/{partition}/messages?offset=0&limit={limit}",
            self.topics(),
            encode_segment(topic)
        )
    }

    pub(crate) fn topic(&self, topic: &str) -> String {
        format!("{}/{}", self.topics(), encode_segment(topic))
    }
}

fn encode_segment(segment: &str) -> String {
    utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string()
}

#[derive(Serialize, Deserialize)]
pub(crate) struct TopicInfo {
    pub(crate) name: String,
    pub(crate) partitions: usize,
    pub(crate) messages: u64,
    #[serde(default, skip_serializing)]
    pub(crate) internal: bool,
}

#[derive(Deserialize)]
struct TopicInfoResponse {
    name: String,
    partition_count: usize,
    total_messages: u64,
    is_internal: bool,
}

pub(crate) fn parse_topics(body: &str) -> Result<Vec<TopicInfo>, String> {
    let topics = serde_json::from_str::<Vec<TopicInfoResponse>>(body).map_err(|e| e.to_string())?;
    Ok(topics
        .into_iter()
        .map(|topic| TopicInfo {
            name: topic.name,
            partitions: topic.partition_count,
            messages: topic.total_messages,
            internal: topic.is_internal,
        })
        .collect())
}

pub(crate) fn user_topics(topics: Vec<TopicInfo>) -> Vec<TopicInfo> {
    topics
        .into_iter()
        .filter(|topic| !topic.internal && validate_user_topic(&topic.name).is_ok())
        .collect()
}

pub(crate) fn validate_user_topic(topic: &str) -> Result<(), String> {
    if topic == "_schemas" || topic.starts_with("__") {
        return Err(format!(
            "Topic '{topic}' is reserved for Streamline internals"
        ));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ProduceRequest {
    records: Vec<ProduceRecord>,
}

#[derive(Serialize, Deserialize)]
struct ProduceRecord {
    key: Option<String>,
    value: serde_json::Value,
    partition: Option<i32>,
    headers: HashMap<String, String>,
}

pub(crate) fn build_produce_request(key: Option<String>, value: String) -> ProduceRequest {
    let value = serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value));
    ProduceRequest {
        records: vec![ProduceRecord {
            key,
            value,
            partition: None,
            headers: HashMap::new(),
        }],
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ConsumedMessage {
    pub(crate) key: String,
    pub(crate) value: String,
    pub(crate) partition: i32,
    pub(crate) offset: i64,
}

#[derive(Deserialize)]
struct ConsumeResponse {
    partition: i32,
    records: Vec<ConsumeRecord>,
}

#[derive(Deserialize)]
struct ConsumeRecord {
    key: Option<String>,
    value: serde_json::Value,
    offset: i64,
}

pub(crate) fn parse_consumed_messages(body: &str) -> Result<Vec<ConsumedMessage>, String> {
    let response = serde_json::from_str::<ConsumeResponse>(body).map_err(|e| e.to_string())?;
    let partition = response.partition;
    response
        .records
        .into_iter()
        .map(|record| {
            let value = match record.value {
                serde_json::Value::String(value) => value,
                value => serde_json::to_string(&value).map_err(|e| e.to_string())?,
            };
            Ok(ConsumedMessage {
                key: record.key.unwrap_or_default(),
                value,
                partition,
                offset: record.offset,
            })
        })
        .collect()
}

pub(crate) fn merge_partition_messages(
    partitions: Vec<Vec<ConsumedMessage>>,
    limit: usize,
) -> Vec<ConsumedMessage> {
    let mut partitions: Vec<VecDeque<ConsumedMessage>> =
        partitions.into_iter().map(VecDeque::from).collect();
    let mut messages = Vec::with_capacity(limit);
    while messages.len() < limit {
        let mut found_message = false;
        for partition in &mut partitions {
            if let Some(message) = partition.pop_front() {
                messages.push(message);
                found_message = true;
                if messages.len() == limit {
                    break;
                }
            }
        }
        if !found_message {
            break;
        }
    }
    messages
}

pub(crate) fn rotated_partition_order(partition_count: usize, start: usize) -> Vec<usize> {
    (0..partition_count)
        .map(|offset| (start + offset) % partition_count)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::TopicApiPaths;

    #[test]
    fn topic_paths_preserve_the_core_contract() {
        let paths = TopicApiPaths::new("http://127.0.0.1:9094/");
        assert_eq!(paths.topics(), "http://127.0.0.1:9094/api/v1/topics");
        assert_eq!(
            paths.messages("orders/eu"),
            "http://127.0.0.1:9094/api/v1/topics/orders%2Feu/messages"
        );
        assert_eq!(
            paths.partition_messages("orders", 2, 50),
            "http://127.0.0.1:9094/api/v1/topics/orders/partitions/2/messages?offset=0&limit=50"
        );
    }
}
