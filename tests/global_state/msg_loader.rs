use super::msg_consumer::{Simple, SimpleConsumer, SimpleMessageHandler};
use rdkafka::consumer::StreamConsumer;
use rdkafka::message::{BorrowedMessage, Message};
use rollup_state_manager::test_utils::messages::{parse_msg, WrappedMessage};
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

const BALANCES_TOPIC: &str = "balances";
const ORDERS_TOPIC: &str = "orders";
const TRADES_TOPIC: &str = "trades";

pub fn load_msgs_from_mq(
    brokers: &str,
    sender: crossbeam_channel::Sender<WrappedMessage>,
) -> Option<std::thread::JoinHandle<anyhow::Result<()>>> {
    let brokers = brokers.to_owned();
    Some(std::thread::spawn(move || {
        let rt: tokio::runtime::Runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        let writer = MessageWriter { sender };
        rt.block_on(async move {
            let consumer: StreamConsumer = rdkafka::config::ClientConfig::new()
                .set("bootstrap.servers", brokers)
                .set("group.id", "unify_msg_dumper")
                .set("enable.partition.eof", "false")
                .set("session.timeout.ms", "6000")
                .set("enable.auto.commit", "true")
                .set("auto.offset.reset", "earliest")
                .create()
                .unwrap();

            let consumer = std::sync::Arc::new(consumer);
            loop {
                let cr_main = SimpleConsumer::new(consumer.as_ref())
                    .add_topic(BALANCES_TOPIC, Simple::from(&writer))
                    .unwrap()
                    .add_topic(ORDERS_TOPIC, Simple::from(&writer))
                    .unwrap()
                    .add_topic(TRADES_TOPIC, Simple::from(&writer))
                    .unwrap();

                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        log::info!("Ctrl-c received, shutting down");
                        break;
                    },

                    err = cr_main.run_stream(|cr|cr.stream()) => {
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
        let message = match msg_type {
            BALANCES_TOPIC => {
                let data = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::BALANCE(data)
            }
            ORDERS_TOPIC => {
                let data = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::ORDER(data)
            }
            TRADES_TOPIC => {
                let data = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::TRADE(data)
            }
            _ => unreachable!(),
        };

        self.sender.try_send(message).unwrap();
    }
}
