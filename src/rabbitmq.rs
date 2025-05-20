use anyhow::Result;
use lapin::{
    options::{BasicPublishOptions, ExchangeDeclareOptions},
    types::{AMQPValue, FieldTable},
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};
use opentelemetry::global;
use serde::Serialize;
use tracing::{debug_span, instrument, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::config::RabbitMQSettings;

#[instrument(name = "rabbitmq_connect", skip_all)]
pub async fn connect(settings: &RabbitMQSettings) -> Result<(Connection, Channel)> {
    let connection = Connection::connect(&settings.url, ConnectionProperties::default()).await?;
    let channel = connection.create_channel().await?;

    Ok((connection, channel))
}

#[instrument(name = "declare_exchange", skip_all)]
pub async fn declare_exchange(channel: &Channel, exchange_name: &str) -> Result<()> {
    channel
        .exchange_declare(
            exchange_name,
            ExchangeKind::Direct,
            ExchangeDeclareOptions {
                durable: true,
                ..ExchangeDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await?;

    Ok(())
}

struct HeaderInjector<'a> {
    headers: &'a mut FieldTable,
}

impl<'a> opentelemetry::propagation::Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        self.headers
            .insert(key.into(), AMQPValue::LongString(value.into()));
    }
}

#[instrument(name = "publish_message", skip_all, fields(exchange = %exchange, routing_key = %routing_key))]
pub async fn publish_message<T: Serialize>(
    channel: &Channel,
    exchange: &str,
    routing_key: &str,
    payload: &T,
) -> Result<()> {
    let payload = serde_json::to_vec(payload)?;

    let mut headers = FieldTable::default();
    let current_context = tracing::Span::current().context();

    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(
            &current_context,
            &mut HeaderInjector {
                headers: &mut headers,
            },
        );
    });

    let span = debug_span!(
        "rabbitmq_publish",
        exchange = %exchange,
        routing_key = %routing_key,
    );

    channel
        .basic_publish(
            exchange,
            routing_key,
            BasicPublishOptions::default(),
            &payload,
            BasicProperties::default()
                .with_delivery_mode(2) // persistent
                .with_headers(headers)
                .with_content_type("application/json".into()),
        )
        .instrument(span)
        .await?;

    Ok(())
}
