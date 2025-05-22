use anyhow::Result;
use lapin::{
    options::{
        BasicConsumeOptions, BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    types::{AMQPValue, FieldTable},
    BasicProperties, Channel, Connection, ConnectionProperties, Consumer, ExchangeKind,
};
use opentelemetry::global;
use serde::Serialize;
use tracing::{debug_span, instrument, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::config::RabbitMQSettings;

#[instrument(name = "rabbitmq_connect", skip_all)]
pub async fn connect(settings: &RabbitMQSettings) -> Result<Connection> {
    let connection = Connection::connect(&settings.url, ConnectionProperties::default()).await?;

    Ok(connection)
}

#[instrument(name = "declare_exchange", skip(channel))]
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

#[instrument(name = "declare_queue", skip(channel))]
pub async fn declare_queue(channel: &Channel, queue_name: &str) -> Result<()> {
    channel
        .queue_declare(
            queue_name,
            QueueDeclareOptions {
                durable: true,
                ..QueueDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await?;

    Ok(())
}

#[instrument(name = "bind_queue", skip(channel))]
pub async fn bind_queue(channel: &Channel, exchange_name: &str, queue_name: &str) -> Result<()> {
    channel
        .queue_bind(
            queue_name,
            exchange_name,
            queue_name,
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    Ok(())
}

#[instrument(name = "declare_and_bind_queue", skip(channel))]
pub async fn declare_and_bind_queue(
    channel: &Channel,
    queue_name: &str,
    exchange_name: &str,
) -> Result<()> {
    declare_queue(channel, queue_name).await?;
    bind_queue(channel, exchange_name, queue_name).await?;

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

#[instrument(name = "create_consumer", skip_all, fields(queue = %queue_name))]
pub async fn create_consumer(channel: &Channel, queue_name: &str) -> Result<Consumer> {
    let consumer = channel
        .basic_consume(
            queue_name,
            "",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    Ok(consumer)
}
