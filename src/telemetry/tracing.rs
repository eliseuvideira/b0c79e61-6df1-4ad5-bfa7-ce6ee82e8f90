use anyhow::Result;
use opentelemetry::{
    propagation::TextMapCompositePropagator,
    trace::{TraceContextExt, TracerProvider},
    KeyValue,
};
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    propagation::{BaggagePropagator, TraceContextPropagator},
    trace::{SdkTracerProvider, Tracer},
    Resource,
};
use opentelemetry_semantic_conventions::resource;
use tokio::{spawn, task::JoinHandle};
use tracing::{level_filters::LevelFilter, Span, Subscriber};
use tracing_loki::BackgroundTask;
use tracing_opentelemetry::{OpenTelemetryLayer, OpenTelemetrySpanExt};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};
use tracing_subscriber::{registry::LookupSpan, Layer};
use url::Url;

pub struct TracingGuard {
    tracer_provider: SdkTracerProvider,
    loki_handle: JoinHandle<()>,
}

impl TracingGuard {
    pub fn tracer_provider(&self) -> &impl TracerProvider {
        &self.tracer_provider
    }

    pub fn loki_handle(&self) -> &JoinHandle<()> {
        &self.loki_handle
    }
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        let _ = self.tracer_provider.force_flush();
        let _ = self.tracer_provider.shutdown();
        self.loki_handle.abort();
    }
}

pub fn init_subscribers() -> Result<TracingGuard> {
    // Filter
    let env_filter = build_env_filter_layer()?;

    // Layers
    let logger_text_layer = build_logger_text_layer();
    let (loki_layer, background_task) = build_loki_layer()?;
    let (otel_layer, tracer_provider) = build_otel_layer()?;

    // Subscriber
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(logger_text_layer)
        .with(loki_layer)
        .with(otel_layer);

    tracing::subscriber::set_global_default(subscriber)?;

    let loki_handle = spawn(background_task);

    Ok(TracingGuard {
        tracer_provider,
        loki_handle,
    })
}

fn build_env_filter_layer() -> Result<EnvFilter> {
    let mut filter = std::env::var("RUST_LOG")
        .or_else(|_| std::env::var("OTEL_LOG_LEVEL"))
        .unwrap_or_else(|_| LevelFilter::INFO.to_string());

    filter.push_str(",otel::tracing=trace");

    Ok(EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .parse_lossy(filter))
}

fn build_logger_text_layer<S>() -> Box<dyn Layer<S> + Send + Sync + 'static>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    Box::new(
        tracing_subscriber::fmt::layer()
            .pretty()
            .with_line_number(true)
            .with_thread_names(true)
            .with_timer(tracing_subscriber::fmt::time::uptime())
            .with_target(true)
            .with_level(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true),
    )
}

fn build_loki_layer() -> Result<(tracing_loki::Layer, BackgroundTask)> {
    let (loki_layer, background_task) = tracing_loki::builder()
        .label("service_name", env!("CARGO_PKG_NAME"))?
        .label("version", env!("CARGO_PKG_VERSION"))?
        .label(
            "environment",
            std::env::var("APP_ENVIRONMENT").unwrap_or_else(|_| "dev".to_string()),
        )?
        .build_url(Url::parse(
            &std::env::var("LOGGING_LOKI_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3100".to_string()),
        )?)?;

    Ok((loki_layer, background_task))
}

fn build_otel_layer<S>() -> Result<(OpenTelemetryLayer<S, Tracer>, SdkTracerProvider)>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let otlp_exporter: opentelemetry_otlp::SpanExporter =
        opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(
                std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                    .unwrap_or_else(|_| "http://127.0.0.1:4317".to_string()),
            )
            .with_protocol(Protocol::Grpc)
            .build()
            .expect("Error");
    let batch_exporter =
        opentelemetry_sdk::trace::BatchSpanProcessor::builder(otlp_exporter).build();
    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(
            Resource::builder()
                .with_attribute(KeyValue::new(
                    resource::SERVICE_NAME,
                    std::env::var("CARGO_PKG_NAME")
                        .unwrap_or_else(|_| "integrations_api".to_string()),
                ))
                .with_attribute(KeyValue::new(
                    resource::SERVICE_VERSION,
                    std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string()),
                ))
                .build(),
        )
        .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
        .with_span_processor(batch_exporter)
        .build();

    use opentelemetry::global;

    init_propagator();

    let layer = tracing_opentelemetry::layer().with_tracer(tracer_provider.tracer(
        std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "integrations_api".to_string()),
    ));
    global::set_tracer_provider(tracer_provider.clone());
    Ok((layer, tracer_provider))
}

fn init_propagator() {
    let propagators = TextMapCompositePropagator::new(vec![
        Box::new(TraceContextPropagator::new()),
        Box::new(BaggagePropagator::new()),
    ]);

    opentelemetry::global::set_text_map_propagator(propagators);
}

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Insert,
    Update,
    Delete,
    Select,
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub fn instrument_query(operation: Operation, table_name: &str) -> tracing::Span {
    tracing::debug_span!(
        "db_query",
        db.system = "postgres",
        db.operation = %operation,
        otel.name = format!("{:?}.{}", operation, table_name),
        otel.kind = "CLIENT",
        otel.status_code = tracing::field::Empty,
    )
}

pub fn propagate_trace_id() {
    let span = Span::current();
    let context = span.context();
    let otel_context = context.span().span_context().clone();
    if otel_context.is_valid() {
        let trace_id = otel_context.trace_id().to_string();
        span.record("trace_id", trace_id);
    }
}
