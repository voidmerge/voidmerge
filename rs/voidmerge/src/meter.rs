//! Metering utilities.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

struct Sys {
    last: std::time::Instant,
    sys_kind: sysinfo::RefreshKind,
    sys: sysinfo::System,
    disk_kind: sysinfo::DiskRefreshKind,
    disks: sysinfo::Disks,
}

impl Default for Sys {
    fn default() -> Self {
        let last = std::time::Instant::now();

        let sys_kind = sysinfo::RefreshKind::nothing()
            .with_cpu(sysinfo::CpuRefreshKind::nothing().with_cpu_usage())
            .with_memory(sysinfo::MemoryRefreshKind::nothing().with_ram());
        let mut sys = sysinfo::System::new_with_specifics(sys_kind);
        sys.refresh_specifics(sys_kind);

        let disk_kind = sysinfo::DiskRefreshKind::nothing().with_storage();
        let disks =
            sysinfo::Disks::new_with_refreshed_list_specifics(disk_kind);

        Sys {
            last,
            sys_kind,
            sys,
            disk_kind,
            disks,
        }
    }
}

impl Sys {
    fn check_update(&mut self) {
        let now = std::time::Instant::now();
        if now - self.last < std::time::Duration::from_secs(10) {
            return;
        }
        self.last = now;
        self.sys.refresh_specifics(self.sys_kind);
        self.disks.refresh_specifics(true, self.disk_kind);
    }

    pub fn mem_avail(&mut self) -> u64 {
        self.check_update();
        self.sys.available_memory()
    }

    pub fn mem_used(&mut self) -> u64 {
        self.check_update();
        self.sys.used_memory()
    }

    pub fn mem_total(&mut self) -> u64 {
        self.check_update();
        self.sys.total_memory()
    }

    pub fn cpu_usage(&mut self) -> f64 {
        self.check_update();
        let mut usage = 0.0_f64;
        for cpu in self.sys.cpus() {
            usage += cpu.cpu_usage() as f64;
        }
        usage / self.sys.cpus().len() as f64
    }

    pub fn disk_total(
        &mut self,
        disk_total_byte: &dyn opentelemetry::metrics::AsyncInstrument<u64>,
    ) {
        self.check_update();
        for disk in self.disks.list() {
            disk_total_byte.observe(
                disk.total_space(),
                &[opentelemetry::KeyValue::new(
                    "mount",
                    disk.mount_point().to_string_lossy().to_string(),
                )],
            );
        }
    }

    pub fn disk_avail(
        &mut self,
        disk_avail_byte: &dyn opentelemetry::metrics::AsyncInstrument<u64>,
    ) {
        self.check_update();
        for disk in self.disks.list() {
            disk_avail_byte.observe(
                disk.available_space(),
                &[opentelemetry::KeyValue::new(
                    "mount",
                    disk.mount_point().to_string_lossy().to_string(),
                )],
            );
        }
    }
}

static SYS: OnceLock<Mutex<Sys>> = OnceLock::new();
fn sys() -> &'static Mutex<Sys> {
    SYS.get_or_init(Default::default)
}

struct OtelMeters {
    fn_gib_sec: opentelemetry::metrics::Counter<f64>,
    egress_gib: opentelemetry::metrics::Counter<f64>,
    storage_gib: opentelemetry::metrics::Gauge<f64>,

    _mem_avail_byte: opentelemetry::metrics::ObservableGauge<u64>,
    _mem_used_byte: opentelemetry::metrics::ObservableGauge<u64>,
    _mem_total_byte: opentelemetry::metrics::ObservableGauge<u64>,

    _cpu_usage_percent: opentelemetry::metrics::ObservableGauge<f64>,

    _disk_total_byte: opentelemetry::metrics::ObservableGauge<u64>,
    _disk_avail_byte: opentelemetry::metrics::ObservableGauge<u64>,
}

impl Default for OtelMeters {
    fn default() -> Self {
        let meter = opentelemetry::global::meter("vm");

        let fn_gib_sec = meter
            .f64_counter("vm.fn")
            .with_unit("GiB-Sec")
            .with_description("Function call memory * duration")
            .build();

        let egress_gib = meter
            .f64_counter("vm.egress")
            .with_unit("GiB")
            .with_description("Egress data transfer")
            .build();

        let storage_gib = meter
            .f64_gauge("vm.obj.storage")
            .with_unit("GiB")
            .with_description("Object storage")
            .build();

        let _mem_avail_byte = meter
            .u64_observable_gauge("vm.sys.mem.avail")
            .with_unit("byte")
            .with_description("Memory (RAM) available")
            .with_callback(|i| {
                i.observe(sys().lock().unwrap().mem_avail(), &[]);
            })
            .build();

        let _mem_used_byte = meter
            .u64_observable_gauge("vm.sys.mem.used")
            .with_unit("byte")
            .with_description("Memory (RAM) used")
            .with_callback(|i| {
                i.observe(sys().lock().unwrap().mem_used(), &[]);
            })
            .build();

        let _mem_total_byte = meter
            .u64_observable_gauge("vm.sys.mem.total")
            .with_unit("byte")
            .with_description("Memory (RAM) total")
            .with_callback(|i| {
                i.observe(sys().lock().unwrap().mem_total(), &[]);
            })
            .build();

        let _cpu_usage_percent = meter
            .f64_observable_gauge("vm.sys.cpu.usage")
            .with_unit("percent")
            .with_description("CPU usage percentage")
            .with_callback(|i| {
                i.observe(sys().lock().unwrap().cpu_usage(), &[]);
            })
            .build();

        let _disk_total_byte = meter
            .u64_observable_gauge("vm.sys.disk.total")
            .with_unit("byte")
            .with_description("Disk total size")
            .with_callback(|i| {
                sys().lock().unwrap().disk_total(i);
            })
            .build();

        let _disk_avail_byte = meter
            .u64_observable_gauge("vm.sys.disk.avail")
            .with_unit("byte")
            .with_description("Disk available size")
            .with_callback(|i| {
                sys().lock().unwrap().disk_avail(i);
            })
            .build();

        Self {
            fn_gib_sec,
            egress_gib,
            storage_gib,
            _mem_avail_byte,
            _mem_used_byte,
            _mem_total_byte,
            _cpu_usage_percent,
            _disk_total_byte,
            _disk_avail_byte,
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
    otel().egress_gib.add(
        egress_gib,
        &[opentelemetry::KeyValue::new("ctx", ctx.to_string())],
    );
    meter_ctx!(ctx).egress_gib += egress_gib;
}

/// Increment the fn memory*duration usage for a context.
pub fn meter_fn_gib_sec(ctx: &Arc<str>, fn_gib_sec: f64) {
    otel().fn_gib_sec.add(
        fn_gib_sec,
        &[opentelemetry::KeyValue::new("ctx", ctx.to_string())],
    );
    meter_ctx!(ctx).fn_gib_sec += fn_gib_sec;
}

/// Set the current storage size for a context.
pub fn meter_storage_gib(ctx: &Arc<str>, storage_gib: f64) {
    otel().storage_gib.record(
        storage_gib,
        &[opentelemetry::KeyValue::new("ctx", ctx.to_string())],
    );
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
