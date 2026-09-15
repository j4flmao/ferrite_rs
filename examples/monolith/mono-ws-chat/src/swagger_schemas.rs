use ferrite_swagger::submit_fragment;
use utoipa::openapi::tag::TagBuilder;
use utoipa::openapi::{ComponentsBuilder, RefOr};
use utoipa::{PartialSchema, ToSchema};

use crate::modules::auth::dto::{AuthResponse, LoginDto, MeResponse, RegisterDto};
use crate::modules::chat::{ChatMessage, SendMessageDto};

fn add_schema<T: ToSchema + PartialSchema>(cb: ComponentsBuilder) -> ComponentsBuilder {
    let name: String = T::name().into_owned();
    let schema: RefOr<utoipa::openapi::schema::Schema> = T::schema();
    cb.schema(name, schema)
}

submit_fragment! { |api: &mut utoipa::openapi::OpenApi| {
    let mut cb = ComponentsBuilder::new();
    macro_rules! add_all {
        ($($t:ty),* $(,)?) => { $( cb = add_schema::<$t>(cb); )* };
    }
    add_all!(RegisterDto, LoginDto, AuthResponse, MeResponse, ChatMessage, SendMessageDto);

    let mut schemas: Vec<(String, RefOr<utoipa::openapi::schema::Schema>)> = Vec::new();
    <RegisterDto as ToSchema>::schemas(&mut schemas);
    <LoginDto as ToSchema>::schemas(&mut schemas);
    <AuthResponse as ToSchema>::schemas(&mut schemas);
    <MeResponse as ToSchema>::schemas(&mut schemas);
    <ChatMessage as ToSchema>::schemas(&mut schemas);
    <SendMessageDto as ToSchema>::schemas(&mut schemas);
    for (name, schema) in schemas {
        cb = cb.schema(name, schema);
    }

    let extra = cb.build();
    let comps = api.components.get_or_insert_with(Default::default);
    comps.schemas.extend(extra.schemas);

    let tags = vec![
        TagBuilder::new().name("Auth").description(Some("Authentication: register, login, JWT bearer tokens." )).build(),
        TagBuilder::new().name("Chat").description(Some("REST history endpoint; realtime chat lives on ws://…/ws/chat.")).build(),
        TagBuilder::new().name("Health").description(Some("Application liveness checks.")).build(),
        TagBuilder::new().name("Swagger").description(Some("OpenAPI document + Swagger UI.")).build(),
    ];
    api.tags = Some(tags);

    fn assign_tag(path: &str) -> Option<&'static str> {
        Some(if path.starts_with("/auth") {
            "Auth"
        } else if path.starts_with("/messages") {
            "Chat"
        } else if path == "/healthz" || path == "/health" || path.starts_with("/health/") {
            "Health"
        } else if path == "/openapi.json" || path.starts_with("/docs") {
            "Swagger"
        } else {
            return None;
        })
    }

    for (path, item) in api.paths.paths.iter_mut() {
        let Some(tag) = assign_tag(path.as_str()) else { continue };
        for op in [
            &mut item.get, &mut item.post, &mut item.put, &mut item.patch,
            &mut item.delete, &mut item.options, &mut item.head, &mut item.trace,
        ].into_iter().flatten() {
            if op.tags.is_none() {
                op.tags = Some(vec![tag.into()]);
            }
        }
    }
}}
