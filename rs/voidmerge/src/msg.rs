//! Message channels.

use crate::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

/// An individual message.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Message {
    /// A message from an application.
    App {
        /// The message payload.
        msg: bytes::Bytes,
    },
    /// A message from a peer client.
    Peer {
        /// The msgId of the remote peer.
        msg_id: Arc<str>,

        /// The message payload.
        msg: bytes::Bytes,
    },
}

/// Message channel receiver.
pub trait MsgRecv: 'static + Send {
    /// Receive a message.
    fn recv(&mut self) -> BoxFut<'_, Option<Message>>;
}

/// Dyn message channel receiver.
pub type DynMsgRecv = Box<dyn MsgRecv + 'static + Send>;

/// Message channels.
pub trait Msg: 'static + Send + Sync {
    /// Construct a new message channel within a context.
    fn create(&self, ctx: Arc<str>) -> BoxFut<'_, Result<Arc<str>>>;

    /// Get a previously created receiver.
    fn get_recv(
        &self,
        ctx: Arc<str>,
        msg_id: Arc<str>,
    ) -> BoxFut<'_, Option<DynMsgRecv>>;

    /// List the active message channels within a context.
    fn list(&self, ctx: Arc<str>) -> BoxFut<'_, Result<Vec<Arc<str>>>>;

    /// Send a message over the channel.
    fn send(
        &self,
        ctx: Arc<str>,
        msg_id: Arc<str>,
        msg: Message,
    ) -> BoxFut<'_, Result<()>>;
}

/// Dyn message channels.
pub type DynMsg = Arc<dyn Msg + 'static + Send + Sync>;

/// Memory-backed message channel.
pub struct MsgMem {
    map: Arc<Mutex<ChanMap>>,
    task: tokio::task::AbortHandle,
}

impl Drop for MsgMem {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl MsgMem {
    /// Construct a new memory-backed message channel.
    pub fn create() -> DynMsg {
        let out = Arc::new_cyclic(|this: &Weak<MsgMem>| {
            let this = this.clone();
            let task = tokio::task::spawn(async move {
                loop {
                    if let Some(this) = this.upgrade() {
                        this.map.lock().unwrap().prune();
                    } else {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(10))
                        .await;
                }
            })
            .abort_handle();
            Self {
                map: ChanMap::new(),
                task,
            }
        });
        let out: DynMsg = out;
        out
    }
}

impl Msg for MsgMem {
    fn create(&self, ctx: Arc<str>) -> BoxFut<'_, Result<Arc<str>>> {
        Box::pin(async move { Ok(self.map.lock().unwrap().msg_new(ctx)) })
    }

    fn get_recv(
        &self,
        ctx: Arc<str>,
        msg_id: Arc<str>,
    ) -> BoxFut<'_, Option<DynMsgRecv>> {
        Box::pin(async move { self.map.lock().unwrap().msg_get(&ctx, &msg_id) })
    }

    fn list(&self, ctx: Arc<str>) -> BoxFut<'_, Result<Vec<Arc<str>>>> {
        Box::pin(async move { Ok(self.map.lock().unwrap().msg_list(&ctx)) })
    }

    fn send(
        &self,
        ctx: Arc<str>,
        msg_id: Arc<str>,
        msg: Message,
    ) -> BoxFut<'_, Result<()>> {
        Box::pin(async move {
            let s = self.map.lock().unwrap().msg_send(&ctx, &msg_id);
            if let Some(s) = s {
                if s.try_send(msg).is_err() {
                    self.map.lock().unwrap().remove(&ctx, &msg_id);
                    Err(Error::other("msg channel closed"))
                } else {
                    Ok(())
                }
            } else {
                Err(Error::other("msg channel closed"))
            }
        })
    }
}

struct ChanItem {
    pub ts: std::time::Instant,
    pub send: tokio::sync::mpsc::Sender<Message>,
    pub recv: Option<DynMsgRecv>,
}

struct ChanMap {
    this: Weak<Mutex<Self>>,
    map: HashMap<Arc<str>, HashMap<Arc<str>, ChanItem>>,
}

impl ChanMap {
    fn new() -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|this| {
            Mutex::new(Self {
                this: this.clone(),
                map: HashMap::new(),
            })
        })
    }

    fn prune(&mut self) {
        self.map.retain(|_, m| {
            m.retain(|_, i| {
                i.recv.is_none()
                    || i.ts.elapsed() < std::time::Duration::from_secs(30)
            });
            !m.is_empty()
        });
    }

    fn msg_new(&mut self, ctx: Arc<str>) -> Arc<str> {
        let mut msg_id = [0; 24];
        use rand::Rng;
        rand::rng().fill(&mut msg_id);
        use base64::prelude::*;
        let msg_id: Arc<str> = BASE64_URL_SAFE_NO_PAD.encode(msg_id).into();
        let (s, r) = tokio::sync::mpsc::channel(32);
        let recv = MsgMemRecv {
            ctx: ctx.clone(),
            msg_id: msg_id.clone(),
            drop: self.this.clone(),
            recv: r,
        };
        let recv: DynMsgRecv = Box::new(recv);
        self.map.entry(ctx).or_default().insert(
            msg_id.clone(),
            ChanItem {
                ts: std::time::Instant::now(),
                send: s,
                recv: Some(recv),
            },
        );
        msg_id
    }

    fn msg_get(
        &mut self,
        ctx: &Arc<str>,
        msg_id: &Arc<str>,
    ) -> Option<DynMsgRecv> {
        if let Some(m) = self.map.get_mut(ctx)
            && let Some(s) = m.get_mut(msg_id)
        {
            return s.recv.take();
        }
        None
    }

    fn msg_list(&self, ctx: &Arc<str>) -> Vec<Arc<str>> {
        if let Some(m) = self.map.get(ctx) {
            return m.keys().cloned().collect();
        }
        vec![]
    }

    fn msg_send(
        &self,
        ctx: &Arc<str>,
        msg_id: &Arc<str>,
    ) -> Option<tokio::sync::mpsc::Sender<Message>> {
        if let Some(m) = self.map.get(ctx)
            && let Some(s) = m.get(msg_id)
        {
            return Some(s.send.clone());
        }
        None
    }

    fn remove(&mut self, ctx: &Arc<str>, msg_id: &Arc<str>) {
        let mut remove_ctx = false;
        if let Some(m) = self.map.get_mut(ctx) {
            m.remove(msg_id);
            if m.is_empty() {
                remove_ctx = true;
            }
        }
        if remove_ctx {
            self.map.remove(ctx);
        }
    }
}

struct MsgMemRecv {
    ctx: Arc<str>,
    msg_id: Arc<str>,
    drop: Weak<Mutex<ChanMap>>,
    recv: tokio::sync::mpsc::Receiver<Message>,
}

impl Drop for MsgMemRecv {
    fn drop(&mut self) {
        if let Some(drop) = self.drop.upgrade() {
            drop.lock().unwrap().remove(&self.ctx, &self.msg_id);
        }
    }
}

impl MsgRecv for MsgMemRecv {
    fn recv(&mut self) -> BoxFut<'_, Option<Message>> {
        Box::pin(async move { self.recv.recv().await })
    }
}
