use crate::test_utils::messages::{parse_msg, WrappedMessage};
use crate::types::matchengine::messages::{DepositMessage, OrderMessage, TradeMessage, UserMessage, WithdrawMessage};
//use fluidex_common::message::consumer::{Simple, SimpleConsumer, SimpleMessageHandler};
use fluidex_common::rdkafka;
use rdkafka::consumer::{Consumer, ConsumerContext, MessageStream, StreamConsumer};
use rdkafka::error::KafkaError;
use rdkafka::message::{BorrowedMessage, Message};
use rdkafka::{Offset, TopicPartitionList};
use std::fs::File;
use std::io::{BufRead, BufReader};
//use std::sync::{Mutex};
use futures::StreamExt;

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
const MSG_TYPE_DEPOSITS: &str = "deposits";
const MSG_TYPE_ORDERS: &str = "orders";
const MSG_TYPE_TRADES: &str = "trades";
const MSG_TYPE_USERS: &str = "registeruser";
const MSG_TYPE_WITHDRAWS: &str = "withdraws";

pub fn load_msgs_from_mq(
    brokers: &str,
    offset: Option<i64>,
    sender: crossbeam_channel::Sender<WrappedMessage>,
) -> Option<std::thread::JoinHandle<anyhow::Result<()>>> {
    let brokers = brokers.to_owned();
    Some(std::thread::spawn(move || {
        let rt: tokio::runtime::Runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();

        let mut writer = MessageWriter {
            sender,
            offset: offset.unwrap_or(-1),
        };
        rt.block_on(async move {
            let mut config = rdkafka::config::ClientConfig::new();
            config
                .set("bootstrap.servers", brokers)
                .set("group.id", "rollup_msg_consumer")
                .set("enable.partition.eof", "false")
                .set("session.timeout.ms", "6000")
                .set("enable.auto.commit", "false");
            if offset.is_none() {
                config.set("auto.offset.reset", "earliest");
            }
            let mut consumer: StreamConsumer = config.create().unwrap();
            loop {
                //alway reset to last offset
                let handle = tokio::runtime::Handle::current();
                let assign_offset = writer.last_offset();
                let join_handle = handle.spawn_blocking(move || {
                    let mut partitions = TopicPartitionList::new();
                    let offset = if assign_offset < 0 {
                        Offset::Beginning
                    } else {
                        Offset::Offset(assign_offset)
                    };
                    log::debug!("assign offset {:?} to consumer", offset);
                    partitions.add_partition_offset(UNIFY_TOPIC, 0, offset).unwrap();
                    consumer.assign(&partitions).unwrap();
                    return consumer;
                });

                consumer = join_handle.await.unwrap();

                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        log::info!("Ctrl-c received, shutting down");
                        return;
                    },

                    err = writer.handle_stream(consumer.stream()) => {
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
    offset: i64,
}

impl MessageWriter {
    fn last_offset(&self) -> i64 {
        self.offset
    }

    async fn handle_stream<C, R>(&mut self, mut strm: MessageStream<'_, C, R>) -> KafkaError
    where
        C: ConsumerContext + 'static,
    {
        loop {
            match strm.next().await.expect("Kafka's stream has no EOF") {
                Err(KafkaError::NoMessageReceived) => {} //nothing to do yet
                Err(KafkaError::PartitionEOF(_)) => {}   //simply omit this type of error
                Err(e) => {
                    return e;
                }
                Ok(m) => self.on_message(&m),
            }
        }
    }

    fn on_message(&mut self, msg: &BorrowedMessage<'_>) {
        let msg_type = std::str::from_utf8(msg.key().unwrap()).unwrap();
        let msg_payload = std::str::from_utf8(msg.payload().unwrap()).unwrap();
        let offset: i64 = msg.offset();
        log::debug!("got message at offset {}", offset);

        let last_offset: i64 = self.offset;
        //tolerance re-winded msg
        if offset <= last_offset {
            return;
        }
        if last_offset != 0 && offset != last_offset + 1 {
            panic!("offset not continuous {} {}", last_offset, offset);
        }
        self.offset = offset;

        let message = match msg_type {
            MSG_TYPE_DEPOSITS => {
                let data: DepositMessage = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::DEPOSIT((data, offset).into())
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
            MSG_TYPE_WITHDRAWS => {
                let data: WithdrawMessage = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::WITHDRAW((data, offset).into())
            }
            _ => return,
        };

        self.sender.try_send(message).unwrap();
    }
}
