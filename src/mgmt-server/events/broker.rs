use std::collections::{hash_map::Entry, HashMap};

use futures::{future, Stream, StreamExt};
use log::warn;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

use super::{Event, TopicName};

pub struct EventBroker {
    topics: RwLock<HashMap<TopicName, broadcast::Sender<Event>>>,
}

impl EventBroker {
    const TOPIC_CAPACITY: usize = 100;

    pub fn new() -> EventBroker {
        EventBroker {
            topics: RwLock::new(HashMap::new()),
        }
    }

    pub async fn publish(&self, event: Event) {
        for topic_name in event.tags.keys() {
            let r_guard = self.topics.read().await;
            if let Some(topic) = r_guard.get(topic_name) {
                // Per https://docs.rs/tokio/1.5.0/tokio/sync/broadcast/struct.Sender.html#method.send,
                // an error will only occur if there are no receivers. This is okay.
                let _ = topic.send(event.clone());
            } else {
                // Write guards needed here, but this only happens once per topic
                // i.e. near the start of the program, when messages start coming in
                std::mem::drop(r_guard);
                self.create_topic_and_publish(topic_name.clone(), event.clone())
                    .await;
            }
        }
    }

    pub async fn subscribe<F>(
        &self,
        topic_name: TopicName,
        filter: F,
    ) -> impl Stream<Item = Event> + Unpin
    where
        F: Fn(&str) -> bool + Clone,
    {
        let rx;
        let r_guard = self.topics.read().await;
        if let Some(topic) = r_guard.get(&topic_name) {
            rx = topic.subscribe();
        } else {
            std::mem::drop(r_guard);
            rx = self.create_topic_with_receiver(topic_name.clone()).await;
        }

        Box::pin(
            BroadcastStream::new(rx)
                .filter_map(move |r| {
                    let filter = filter.clone();
                    let topic_name = topic_name.clone();
                    async move {
                        match r {
                            Ok(event) => {
                                if let Some(v) = event.tags.get(&topic_name) {
                                    filter(v).then_some(event)
                                } else {
                                    None
                                }
                            }
                            Err(BroadcastStreamRecvError::Lagged(skipped)) => {
                                warn!("Subscriber lagged, skipped {} messages", skipped);
                                None
                            }
                        }
                    }
                })
                .map(future::ready)
                .buffered(20),
        )
    }

    async fn create_topic_with_receiver(
        &self,
        topic_name: TopicName,
    ) -> broadcast::Receiver<Event> {
        let mut w_guard = self.topics.write().await;
        match w_guard.entry(topic_name) {
            Entry::Vacant(e) => {
                let (tx, rx) = broadcast::channel(EventBroker::TOPIC_CAPACITY);
                e.insert(tx);
                rx
            }
            Entry::Occupied(o) => o.into_mut().subscribe(),
        }
    }

    async fn create_topic_and_publish(&self, topic_name: TopicName, event: Event) {
        let mut w_guard = self.topics.write().await;
        let sender = match w_guard.entry(topic_name) {
            Entry::Vacant(e) => {
                let (tx, ..) = broadcast::channel(EventBroker::TOPIC_CAPACITY);
                e.insert(tx)
            }
            Entry::Occupied(o) => o.into_mut(),
        };

        // Send while holding the write guard instead of re-acquiring
        let _ = sender.send(event);
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use futures::{pin_mut, FutureExt};
    use tokio_stream::StreamExt;

    use super::*;

    #[tokio::test]
    async fn subscriber_can_receive_published_event() {
        fctrl::util::testing::logger_init();

        let broker = EventBroker::new();

        let topic = TopicName("test_tag".to_owned());
        let test_event_tags = [(topic.clone(), "yes".to_owned())]
            .iter()
            .cloned()
            .collect();
        let test_event = Event {
            tags: test_event_tags,
            timestamp: Utc::now(),
            content: "asdf".to_owned(),
        };

        let s = broker.subscribe(topic, |s| s == "yes").await;
        pin_mut!(s);

        broker.publish(test_event.clone()).await;

        let e = s.next().await.unwrap();
        assert_eq!(e, test_event);
    }

    #[tokio::test]
    async fn subscriber_filters_unwanted_published_event() {
        fctrl::util::testing::logger_init();

        let broker = EventBroker::new();

        let topic = TopicName("test_tag".to_owned());
        let test_event_tags = [(topic.clone(), "yes".to_owned())]
            .iter()
            .cloned()
            .collect();
        let test_event = Event {
            tags: test_event_tags,
            timestamp: Utc::now(),
            content: "aaaa".to_owned(),
        };

        let s = broker.subscribe(topic, |s| s != "yes").await;

        broker.publish(test_event.clone()).await;

        pin_mut!(s);
        assert_eq!(s.next().now_or_never(), None);
    }

    #[tokio::test]
    async fn publishing_to_non_subscribed_topic_drops_events() {
        fctrl::util::testing::logger_init();

        let broker = EventBroker::new();

        let topic = TopicName("test_tag".to_owned());
        let test_event_tags = [(topic.clone(), "yes".to_owned())]
            .iter()
            .cloned()
            .collect();
        let test_event = Event {
            tags: test_event_tags,
            timestamp: Utc::now(),
            content: "bbbb".to_owned(),
        };

        broker.publish(test_event).await;

        let s = broker.subscribe(topic, |_| true).await;
        pin_mut!(s);
        assert_eq!(s.next().now_or_never(), None);
    }
}
