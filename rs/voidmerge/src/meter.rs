//! Metering utilities.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Agg {
    fn_gib_sec: f64,
    egress_gib: f64,
    storage_gib: f64,
}

type AggMap = HashMap<Arc<str>, Agg>;

static METER: OnceLock<Mutex<AggMap>> = OnceLock::new();

fn meter() -> &'static Mutex<AggMap> {
    METER.get_or_init(Default::default)
}

macro_rules! meter_ctx {
    ($ctx: ident) => {
        meter().lock().unwrap().entry($ctx.clone()).or_default()
    };
}

/// Call this once in binary to init metering task.
pub fn meter_init() {
    tokio::task::spawn(init_meter_task());
}

/// Increment the egress usage for a context.
pub fn meter_egress_gib(ctx: &Arc<str>, egress_gib: f64) {
    meter_ctx!(ctx).egress_gib += egress_gib;
}

/// Increment the fn memory*duration usage for a context.
pub fn meter_fn_gib_sec(ctx: &Arc<str>, fn_gib_sec: f64) {
    meter_ctx!(ctx).fn_gib_sec += fn_gib_sec;
}

/// Set the current storage size for a context.
pub fn meter_storage_gib(ctx: &Arc<str>, storage_gib: f64) {
    meter_ctx!(ctx).storage_gib = storage_gib;
}

async fn init_meter_task() {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60 * 5)).await;

        let map: AggMap = std::mem::take(&mut *meter().lock().unwrap());

        for (ctx, meter) in map {
            tracing::info!(
                target: "METER",
                %ctx,
                fnGibSec = meter.fn_gib_sec,
                egressGib = meter.egress_gib,
                storageGib = meter.storage_gib,
            );
        }
    }
}
