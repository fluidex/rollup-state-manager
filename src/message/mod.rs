use anyhow::Result;
use fluidex_common::db::models::account;
use serde::{Deserialize, Serialize};

pub mod consumer;
pub mod persist;
pub mod producer;

pub use producer::{
    BALANCES_TOPIC, DEPOSITS_TOPIC, INTERNALTX_TOPIC, ORDERS_TOPIC, TRADES_TOPIC, UNIFY_TOPIC, USER_TOPIC, WITHDRAWS_TOPIC,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub user_id: u32,
    pub l1_address: String,
    pub l2_pubkey: String,
}

impl From<account::AccountDesc> for UserMessage {
    fn from(user: account::AccountDesc) -> Self {
        Self {
            user_id: user.id as u32,
            l1_address: user.l1_address,
            l2_pubkey: user.l2_pubkey,
        }
    }
}

//TODO: senderstatus is not used anymore?
#[derive(Serialize, Deserialize)]
pub struct MessageSenderStatus {
    trades_len: usize,
    orders_len: usize,
    balances_len: usize,
}

pub trait MessageManager: Sync + Send {
    //fn push_message(&mut self, msg: &Message);
    fn is_block(&self) -> bool;
    fn push_user_message(&mut self, user: &UserMessage);
}

pub struct RdProducerStub<T> {
    pub sender: crossbeam_channel::Sender<(&'static str, String)>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> RdProducerStub<T> {
    fn push_message_and_topic(&self, message: String, topic_name: &'static str) {
        //log::debug!("KAFKA: push {} message: {}", topic_name, message);
        self.sender.try_send((topic_name, message)).unwrap();
    }
}

impl<T: producer::MessageScheme + 'static> RdProducerStub<T> {
    pub fn new_and_run(brokers: &str) -> Result<Self> {
        //now the channel is just need to provide a small buffer which is
        //enough to accommodate a pluse request in some time slice of thread
        let (sender, receiver) = crossbeam_channel::bounded(2048);

        let producer_context: producer::RdProducerContext<T> = Default::default();

        let kafkaproducer = producer_context.new_producer(brokers)?;
        std::thread::spawn(move || {
            producer::RdProducerContext::<T>::run_default(kafkaproducer, receiver);
        });
        Ok(Self {
            sender,
            _phantom: std::marker::PhantomData,
        })
    }
}

impl<T: producer::MessageScheme> MessageManager for RdProducerStub<T> {
    fn is_block(&self) -> bool {
        // https://github.com/fluidex/dingir-exchange/issues/119
        //self.sender.is_full()
        //self.sender.len() >= (self.sender.capacity().unwrap() as f64 * 0.9) as usize
        self.sender.len() >= (self.sender.capacity().unwrap() - 1000)
    }
    fn push_user_message(&mut self, user: &UserMessage) {
        let message = serde_json::to_string(&user).unwrap();
        self.push_message_and_topic(message, USER_TOPIC)
    }
}

pub type SimpleMessageManager = RdProducerStub<producer::SimpleMessageScheme>;

// TODO: since now we enable SimpleMessageManager & FullOrderMessageManager both,
// we only need to process useful (deposit,trade etc, which update the rollup global state) msgs only
// and skip others
pub type FullOrderMessageManager = RdProducerStub<producer::FullOrderMessageScheme>;

// https://rust-lang.github.io/rust-clippy/master/index.html#large_enum_variant
// TODO: better naming?
// TODO: change push_order_message etc interface to this enum class?
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "value")]
pub enum Message {
    UserMessage(Box<UserMessage>),
}

pub fn new_simple_message_manager(brokers: &str) -> Result<SimpleMessageManager> {
    SimpleMessageManager::new_and_run(brokers)
}

pub fn new_full_order_message_manager(brokers: &str) -> Result<FullOrderMessageManager> {
    FullOrderMessageManager::new_and_run(brokers)
}
