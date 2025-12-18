//! A memory-backed object index.

use crate::error::ErrorExt;
use crate::obj::ObjMeta;
use crate::safe_now;
use crate::{Error, Result};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

/// A memory-backed object index.
pub struct MemIndex<Info: Clone> {
    map: OrderMap<(ObjMeta, Info)>,
    delete: Vec<(ObjMeta, Info)>,
}

impl<Info: Clone> Default for MemIndex<Info> {
    fn default() -> Self {
        Self {
            map: Default::default(),
            delete: Default::default(),
        }
    }
}

impl<Info: Clone> MemIndex<Info> {
    /// Get metrics.
    pub fn meter(&self) -> HashMap<Arc<str>, u64> {
        let mut map: HashMap<Arc<str>, u64> = Default::default();
        for (meta, _info) in self.map.iter(f64::MIN, f64::MAX) {
            if meta.sys_prefix() != ObjMeta::SYS_CTX {
                continue;
            }
            *map.entry(meta.ctx().into()).or_default() += meta.byte_length();
        }
        map
    }

    /// After any mutation operation, if there are items to delete,
    /// they will be listed here.
    pub fn get_delete(&mut self) -> Vec<(ObjMeta, Info)> {
        std::mem::take(&mut self.delete)
    }

    /// Prune expired items.
    pub fn prune(&mut self) {
        let now = safe_now();
        self.map.retain(|_, (meta, info)| {
            let x = meta.expires_secs();
            if x == 0.0 || x > now {
                true
            } else {
                self.delete.push((meta.clone(), info.clone()));
                false
            }
        });
    }

    /// Get an item from the index.
    pub fn get(&self, meta: ObjMeta) -> Result<(ObjMeta, Info)> {
        let pfx = Pfx::new(&meta);
        self.map.get(&pfx).cloned().ok_or_else(|| {
            Error::not_found(format!("Could not locate meta: {meta}"))
        })
    }

    /// List items in the index.
    pub fn list(
        &self,
        prefix: Arc<str>,
        created_gt: f64,
        limit: u32,
    ) -> Vec<Arc<str>> {
        let mut out = Vec::new();
        let mut last_created_secs = 0.0;
        for (meta, _info) in self.map.iter(created_gt, f64::MAX) {
            let created_secs = meta.created_secs();
            if out.len() >= limit as usize && created_secs > last_created_secs {
                // in edge case of exactly matching created_secs, we may return
                // more than the limit, but if we don't do this, the continue
                // token will cause them to miss some items
                return out;
            }
            last_created_secs = created_secs;
            if created_secs > created_gt && meta.0.starts_with(&*prefix) {
                out.push(meta.0.clone());
            }
        }
        out
    }

    /// Put an item into the index.
    pub fn put(&mut self, meta: ObjMeta, info: Info) {
        let now = safe_now();
        let mx = meta.expires_secs();
        if mx > 0.0 && mx < now {
            self.delete.push((meta, info));
            return;
        }
        let pfx = Pfx::new(&meta);
        let created_secs = meta.created_secs();
        if let Some((orig_meta, orig_info)) =
            self.map.insert(created_secs, pfx, (meta, info))
        {
            let ox = orig_meta.expires_secs();
            if ox > 0.0 && ox < now {
                self.delete.push((orig_meta, orig_info));
                return;
            }
            let orig_created_secs = orig_meta.created_secs();
            if orig_created_secs >= created_secs {
                // woops, put it back
                if let Some((meta, info)) = self.map.insert(
                    orig_created_secs,
                    Pfx::new(&orig_meta),
                    (orig_meta, orig_info),
                ) {
                    self.delete.push((meta, info));
                }
            } else {
                self.delete.push((orig_meta, orig_info));
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Order(f64);

impl PartialEq for Order {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for Order {}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Pfx(Arc<str>);

impl Pfx {
    pub fn new(meta: &ObjMeta) -> Self {
        Self(format!(
            "{}/{}/{}",
            meta.sys_prefix(),
            meta.ctx(),
            meta.app_path(),
        ).into())
    }
}

struct OrderMap<T> {
    map: HashMap<Pfx, (Order, T)>,
    order: BTreeMap<Order, HashSet<Pfx>>,
}

impl<T> Default for OrderMap<T> {
    fn default() -> Self {
        Self {
            map: Default::default(),
            order: Default::default(),
        }
    }
}

impl<T> OrderMap<T> {
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&Pfx, &T) -> bool,
    {
        let mut remove = Vec::new();
        for (pfx, (_, t)) in self.map.iter() {
            if !f(pfx, t) {
                remove.push(pfx.clone());
            }
        }
        for pfx in remove {
            self.remove(&pfx);
        }
    }

    pub fn remove(&mut self, pfx: &Pfx) -> Option<T> {
        if let Some((order, t)) = self.map.remove(pfx) {
            let mut remove = false;
            if let Some(set) = self.order.get_mut(&order) {
                set.remove(pfx);
                if set.is_empty() {
                    remove = true;
                }
            }
            if remove {
                self.order.remove(&order);
            }
            Some(t)
        } else {
            None
        }
    }

    pub fn insert(&mut self, order: f64, pfx: Pfx, val: T) -> Option<T> {
        let out = self.remove(&pfx);
        let order = Order(order);
        self.map.insert(pfx.clone(), (order, val));
        self.order.entry(order).or_default().insert(pfx);
        out
    }

    pub fn get(&self, pfx: &Pfx) -> Option<&T> {
        self.map.get(pfx).map(|v| &v.1)
    }

    pub fn iter(&self, start: f64, end: f64) -> impl Iterator<Item = &T> {
        self.order
            .range(Order(start)..Order(end))
            .flat_map(|(_, set)| {
                set.iter().filter_map(|pfx| self.map.get(pfx).map(|v| &v.1))
            })
    }
}
