use crate::message::{Message, MessageManager};
use fluidex_common::db::models::account;
use super::history::HistoryWriter;

///////////////////////////// PersistExector interface ////////////////////////////

// TODO: fix methods, use ref or value?
pub trait PersistExector: Send + Sync {
    fn service_available(&self) -> bool {
        true
    }
    // make sure all data has been persisted
    //fn flush(&self) {
    //}
    fn real_persist(&self) -> bool {
        true
    }
    fn register_user(&mut self, user: account::AccountDesc);
}

impl PersistExector for Box<dyn PersistExector + '_> {
    fn service_available(&self) -> bool {
        self.as_ref().service_available()
    }
    fn real_persist(&self) -> bool {
        self.as_ref().real_persist()
    }
    fn register_user(&mut self, user: account::AccountDesc) {
        self.as_mut().register_user(user)
    }
}

impl PersistExector for &mut Box<dyn PersistExector + '_> {
    fn service_available(&self) -> bool {
        self.as_ref().service_available()
    }
    fn real_persist(&self) -> bool {
        self.as_ref().real_persist()
    }
    fn register_user(&mut self, user: account::AccountDesc) {
        self.as_mut().register_user(user)
    }
}

///////////////////////////// DummyPersistor  ////////////////////////////

// do nothing

#[derive(Default)]
pub struct DummyPersistor {}
impl DummyPersistor {
    pub fn new() -> Self {
        DummyPersistor {}
    }
    pub fn new_box() -> Box<Self> {
        Box::new(DummyPersistor {})
    }
}
impl PersistExector for DummyPersistor {
    fn real_persist(&self) -> bool {
        false
    }
    fn register_user(&mut self, _user: account::AccountDesc) {}
}

impl PersistExector for &mut DummyPersistor {
    fn real_persist(&self) -> bool {
        false
    }
    fn register_user(&mut self, _user: account::AccountDesc) {}
}

///////////////////////////// MemBasedPersistor ////////////////////////////

#[derive(Default)]
pub struct MemBasedPersistor {
    pub messages: Vec<Message>,
}
impl MemBasedPersistor {
    pub fn new() -> Self {
        Self { messages: Vec::new() }
    }
}

impl PersistExector for MemBasedPersistor {
    fn register_user(&mut self, user: account::AccountDesc) {
        self.messages.push(Message::UserMessage(Box::new(user.into())));
    }
}

///////////////////////////// FileBasedPersistor ////////////////////////////

pub struct FileBasedPersistor {
    output_file: std::fs::File,
}
impl FileBasedPersistor {
    pub fn new(output_file_name: &str) -> Self {
        let output_file = std::fs::File::create(output_file_name).unwrap();
        Self { output_file }
    }
    pub fn write_msg(&mut self, msg: Message) {
        use std::io::Write;
        let s = serde_json::to_string(&msg).unwrap();
        self.output_file.write_fmt(format_args!("{}\n", s)).unwrap();
    }
}

impl PersistExector for FileBasedPersistor {
    fn register_user(&mut self, user: account::AccountDesc) {
        let msg = Message::UserMessage(Box::new(user.into()));
        self.write_msg(msg);
    }
}

///////////////////////////// MessengerBasedPersistor  ////////////////////////////

pub struct MessengerBasedPersistor {
    inner: Box<dyn MessageManager>,
}

impl MessengerBasedPersistor {
    pub fn new(inner: Box<dyn MessageManager>) -> Self {
        Self { inner }
    }
}

impl PersistExector for MessengerBasedPersistor {
    fn service_available(&self) -> bool {
        if self.inner.is_block() {
            log::warn!("message_manager full");
            return false;
        }
        true
    }
    fn register_user(&mut self, user: account::AccountDesc) {
        self.inner.push_user_message(&user.into());
    }
}

///////////////////////////// DBBasedPersistor  ////////////////////////////
///
pub struct DBBasedPersistor {
    inner: Box<dyn HistoryWriter>,
}

impl DBBasedPersistor {
    pub fn new(inner: Box<dyn HistoryWriter>) -> Self {
        Self { inner }
    }
}

impl PersistExector for DBBasedPersistor {
    fn service_available(&self) -> bool {
        if self.inner.is_block() {
            log::warn!("history_writer full");
            return false;
        }
        true
    }
    fn register_user(&mut self, user: account::AccountDesc) {
        self.inner.append_user(user);
    }
}

///////////////////////////// CompositePersistor  ////////////////////////////
///
#[derive(Default)]
pub struct CompositePersistor {
    persistors: Vec<Box<dyn PersistExector>>,
}

impl CompositePersistor {
    pub fn add_persistor(&mut self, p: Box<dyn PersistExector>) {
        self.persistors.push(p)
    }
}

impl PersistExector for CompositePersistor {
    fn service_available(&self) -> bool {
        for p in &self.persistors {
            if !p.service_available() {
                return false;
            }
        }
        true
    }
    fn register_user(&mut self, user: account::AccountDesc) {
        for p in &mut self.persistors {
            p.register_user(user.clone());
        }
    }
}
