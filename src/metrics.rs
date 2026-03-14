use prometheus::{
    Counter, CounterVec, Histogram, HistogramOpts, IntGauge, Opts, Registry, TextEncoder,
};

#[derive(Clone)]
pub struct Metrics {
    pub requests_total: Counter,
    pub requests_failed: Counter,
    pub backend_errors: CounterVec,
    pub active_connections: IntGauge,
    pub request_duration_seconds: Histogram,
    registry: Registry,
}

impl Metrics {
    pub fn new(backend_addrs: &[std::net::SocketAddr]) -> Self {
        let registry = Registry::new();

        let requests_total =
            Counter::with_opts(Opts::new("requests_total", "Total proxied requests")).unwrap();
        let requests_failed =
            Counter::with_opts(Opts::new("requests_failed", "Requests that errored")).unwrap();
        let backend_errors = CounterVec::new(
            Opts::new("backend_errors", "Errors per backend"),
            &["backend"],
        )
        .unwrap();
        let active_connections = IntGauge::with_opts(Opts::new(
            "active_connections",
            "Currently active connections",
        ))
        .unwrap();
        let request_duration_seconds = Histogram::with_opts(
            HistogramOpts::new("request_duration_seconds", "Request latency distribution").buckets(
                vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 5.0],
            ),
        )
        .unwrap();

        registry.register(Box::new(requests_total.clone())).unwrap();
        registry
            .register(Box::new(requests_failed.clone()))
            .unwrap();
        registry.register(Box::new(backend_errors.clone())).unwrap();
        registry
            .register(Box::new(active_connections.clone()))
            .unwrap();
        registry
            .register(Box::new(request_duration_seconds.clone()))
            .unwrap();

        for addr in backend_addrs {
            backend_errors.with_label_values(&[&addr.to_string()]);
        }

        Self {
            requests_total,
            requests_failed,
            backend_errors,
            active_connections,
            request_duration_seconds,
            registry,
        }
    }

    pub fn encode(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode_to_string(&metric_families).unwrap()
    }
}
