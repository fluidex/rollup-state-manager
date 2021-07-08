use super::msg_consumer::{Simple, SimpleConsumer, SimpleMessageHandler};
use crate::test_utils::messages::{parse_msg, WrappedMessage};
use crate::types::matchengine::messages::{BalanceMessage, OrderMessage, TradeMessage, UserMessage};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{BorrowedMessage, Message};
use rdkafka::{Offset, TopicPartitionList};
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn load_msgs_from_file(
    filepath: &str,
    sender: crossbeam_channel::Sender<WrappedMessage>,
) -> Option<std::thread::JoinHandle<anyhow::Result<()>>> {
    let filepath = filepath.to_string();
    println!("loading from {}", filepath);
    Some(std::thread::spawn(move || {
        let file = File::open(filepath)?;
        // since
        for l in BufReader::new(file).lines() {
            let msg = parse_msg(l?).expect("invalid data");
            sender.try_send(msg)?;
        }
        Ok(())
    }))
}

const UNIFY_TOPIC: &str = "unifyevents";
const MSG_TYPE_BALANCES: &str = "balances";
const MSG_TYPE_USERS: &str = "registeruser";
const MSG_TYPE_ORDERS: &str = "orders";
const MSG_TYPE_TRADES: &str = "trades";

pub fn load_msgs_from_mq(
    brokers: &str,
    offset: Option<i64>,
    sender: crossbeam_channel::Sender<WrappedMessage>,
) -> Option<std::thread::JoinHandle<anyhow::Result<()>>> {
    let brokers = brokers.to_owned();
    Some(std::thread::spawn(move || {
        let rt: tokio::runtime::Runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();

        let writer = MessageWriter { sender };
        rt.block_on(async move {
            let consumer: StreamConsumer = rdkafka::config::ClientConfig::new()
                .set("bootstrap.servers", brokers)
                .set("group.id", "rollup_msg_consumer")
                .set("enable.partition.eof", "false")
                .set("session.timeout.ms", "6000")
                .set("enable.auto.commit", "false")
                .create()
                .unwrap();

            let mut partitions = TopicPartitionList::new();
            partitions.add_partition(UNIFY_TOPIC, 0);
            if let Some(offset) = offset {
                // FIXME: this might panic if there is no new message, fallback in this scenario
                partitions.set_partition_offset(UNIFY_TOPIC, 0, Offset::Offset(offset + 1)).unwrap();
            } else {
                partitions.set_partition_offset(UNIFY_TOPIC, 0, Offset::Offset(0)).unwrap();
            }
            consumer.assign(&partitions).unwrap();

            let consumer = std::sync::Arc::new(consumer);
            loop {
                let cr_main = SimpleConsumer::new(consumer.as_ref())
                    .add_topic(UNIFY_TOPIC, Simple::from(&writer))
                    .unwrap();

                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        log::info!("Ctrl-c received, shutting down");
                        break;
                    },

                    err = cr_main.run_stream(|cr| cr.stream()) => {
                        log::error!("Kafka consumer error: {}", err);
                    }
                }
            }
        });

        Ok(())
    }))
}

struct MessageWriter {
    sender: crossbeam_channel::Sender<WrappedMessage>,
}

impl SimpleMessageHandler for &MessageWriter {
    fn on_message(&self, msg: &BorrowedMessage<'_>) {
        let msg_type = std::str::from_utf8(msg.key().unwrap()).unwrap();
        let msg_payload = std::str::from_utf8(msg.payload().unwrap()).unwrap();
        let offset = msg.offset();
        log::debug!("got message at offset {}", offset);
        let message = match msg_type {
            MSG_TYPE_BALANCES => {
                let data: BalanceMessage = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::BALANCE((data, offset).into())
            }
            MSG_TYPE_ORDERS => {
                let data: OrderMessage = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::ORDER((data, offset).into())
            }
            MSG_TYPE_TRADES => {
                let data: TradeMessage = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::TRADE((data, offset).into())
            }
            MSG_TYPE_USERS => {
                let data: UserMessage = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::USER((data, offset).into())
            }
            _ => unreachable!(),
        };

        self.sender.try_send(message).unwrap();
    }
}
