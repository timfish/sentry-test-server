use actix_web::{
    post,
    web::{self, Bytes},
    App, HttpResponse, HttpServer, Responder,
};
use crossbeam_channel::{Receiver, Sender};
use sentry_types::protocol::v7::Envelope;
use sentry_types::protocol::v7::*;
use serde::Serialize;
use std::net;

struct AppState {
    envelope_tx: Sender<Envelope>,
}

#[post("/api/{_project_id}/envelope/")]
async fn envelope(
    _project_id: web::Path<String>,
    req_body: Bytes,
    state: web::Data<AppState>,
) -> impl Responder {
    let envelope = Envelope::from_slice(&req_body).expect("invalid envelope");

    state
        .envelope_tx
        .send(envelope)
        .expect("could not send envelope");

    HttpResponse::Ok()
}

pub fn server<A>(address: A) -> std::io::Result<Receiver<Envelope>>
where
    A: net::ToSocketAddrs,
{
    let (envelope_tx, envelope_rx) = crossbeam_channel::bounded(1);

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState {
                envelope_tx: envelope_tx.clone(),
            }))
            .route("/", web::to(HttpResponse::Ok))
            .service(envelope)
    })
    .bind(address)?
    .run();

    actix_rt::spawn(async move { server.await });

    Ok(envelope_rx)
}

pub fn to_json(env: &Envelope) -> serde_json::Result<String> {
    serde_json::to_string(
        &env.items()
            .map(EnvelopeItemSerialisable::from)
            .collect::<Vec<_>>(),
    )
}

pub fn to_json_pretty(env: &Envelope) -> serde_json::Result<String> {
    serde_json::to_string_pretty(
        &env.items()
            .map(EnvelopeItemSerialisable::from)
            .collect::<Vec<_>>(),
    )
}

// Since these type are not serialisable in sentry-types, we need to duplicate them here.

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize)]
#[serde(rename = "Attachment")]
/// Represents an attachment item.
pub struct AttachmentSerialisable {
    /// The actual attachment data.
    pub length: u64,
    /// The filename of the attachment.
    pub filename: String,
    /// The Content Type of the attachment
    pub content_type: Option<String>,
    /// The special type of this attachment.
    pub ty: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[non_exhaustive]
#[allow(clippy::large_enum_variant)]
#[serde(rename = "EnvelopeItem")]
pub enum EnvelopeItemSerialisable {
    /// An Event Item.
    ///
    /// See the [Event Item documentation](https://develop.sentry.dev/sdk/envelopes/#event)
    /// for more details.
    Event(Event<'static>),
    /// A Session Item.
    ///
    /// See the [Session Item documentation](https://develop.sentry.dev/sdk/envelopes/#session)
    /// for more details.
    SessionUpdate(SessionUpdate<'static>),
    /// A Session Aggregates Item.
    ///
    /// See the [Session Aggregates Item documentation](https://develop.sentry.dev/sdk/envelopes/#sessions)
    /// for more details.
    SessionAggregates(SessionAggregates<'static>),
    /// A Transaction Item.
    ///
    /// See the [Transaction Item documentation](https://develop.sentry.dev/sdk/envelopes/#transaction)
    /// for more details.
    Transaction(Transaction<'static>),
    /// An Attachment Item.
    ///
    /// See the [Attachment Item documentation](https://develop.sentry.dev/sdk/envelopes/#attachment)
    /// for more details.
    Attachment(AttachmentSerialisable),
    // TODO:
    // etc…
}

impl From<&EnvelopeItem> for EnvelopeItemSerialisable {
    fn from(item: &EnvelopeItem) -> Self {
        match item {
            EnvelopeItem::Event(event) => Self::Event(event.clone()),
            EnvelopeItem::SessionUpdate(session) => Self::SessionUpdate(session.clone()),
            EnvelopeItem::SessionAggregates(session) => Self::SessionAggregates(session.clone()),
            EnvelopeItem::Transaction(transaction) => Self::Transaction(transaction.clone()),
            EnvelopeItem::Attachment(attachment) => Self::Attachment(AttachmentSerialisable {
                length: attachment.buffer.len() as u64,
                filename: attachment.filename.clone(),
                content_type: attachment.content_type.clone(),
                ty: attachment.ty.map(|ty| ty.as_str().to_string()),
            }),
            _ => todo!(),
        }
    }
}
