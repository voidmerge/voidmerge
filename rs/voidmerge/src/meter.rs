//! Metering utilities.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

struct OtelMeters {
    fn_gib_sec: opentelemetry::metrics::Counter<f64>,
    egress_gib: opentelemetry::metrics::Counter<f64>,
    storage_gib: opentelemetry::metrics::Gauge<f64>,
}

impl Default for OtelMeters {
    fn default() -> Self {
        let meter = opentelemetry::global::meter("vm");

        let fn_gib_sec = meter.f64_counter("vm.fn")
            .with_unit("GiB-Sec")
            .build();

        let egress_gib = meter.f64_counter("vm.egress")
            .with_unit("GiB")
            .build();

        let storage_gib = meter.f64_gauge("vm.obj.storage")
            .with_unit("GiB")
            .build();

        Self {
            fn_gib_sec,
            egress_gib,
            storage_gib,
        }
    }
}

static OTEL_METERS: OnceLock<OtelMeters> = OnceLock::new();
fn otel() -> &'static OtelMeters {
    OTEL_METERS.get_or_init(Default::default)
}

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
    // initialize the otel meters
    otel();
    tokio::task::spawn(init_meter_task());
}

/// Increment the egress usage for a context.
pub fn meter_egress_gib(ctx: &Arc<str>, egress_gib: f64) {
    otel().egress_gib.add(egress_gib, &[
        opentelemetry::KeyValue::new("ctx", ctx.to_string()),
    ]);
    meter_ctx!(ctx).egress_gib += egress_gib;
}

/// Increment the fn memory*duration usage for a context.
pub fn meter_fn_gib_sec(ctx: &Arc<str>, fn_gib_sec: f64) {
    otel().fn_gib_sec.add(fn_gib_sec, &[
        opentelemetry::KeyValue::new("ctx", ctx.to_string()),
    ]);
    meter_ctx!(ctx).fn_gib_sec += fn_gib_sec;
}

/// Set the current storage size for a context.
pub fn meter_storage_gib(ctx: &Arc<str>, storage_gib: f64) {
    otel().storage_gib.record(storage_gib, &[
        opentelemetry::KeyValue::new("ctx", ctx.to_string()),
    ]);
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
