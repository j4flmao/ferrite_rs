use ferrite_swagger::submit_fragment;
use utoipa::openapi::tag::TagBuilder;
use utoipa::openapi::{ComponentsBuilder, RefOr};
use utoipa::{PartialSchema, ToSchema};

use crate::auth::dto::{AuthResponse, LoginDto, RegisterDto};
use crate::carts::dto::{AddItemDto, UpdateQtyDto};
use crate::categories::dto::{CategoryResponse, CreateCategoryDto, UpdateCategoryDto};
use crate::orders::dto::CheckoutDto;
use crate::products::dto::{CreateProductDto, ProductResponse, UpdateProductDto};

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
    add_all!(
        RegisterDto, LoginDto, AuthResponse,
        CreateCategoryDto, UpdateCategoryDto, CategoryResponse,
        CreateProductDto, UpdateProductDto, ProductResponse,
        AddItemDto, UpdateQtyDto, CheckoutDto,
    );

    let mut schemas: Vec<(String, RefOr<utoipa::openapi::schema::Schema>)> = Vec::new();
    <RegisterDto as ToSchema>::schemas(&mut schemas);
    <LoginDto as ToSchema>::schemas(&mut schemas);
    <AuthResponse as ToSchema>::schemas(&mut schemas);
    <CreateCategoryDto as ToSchema>::schemas(&mut schemas);
    <UpdateCategoryDto as ToSchema>::schemas(&mut schemas);
    <CategoryResponse as ToSchema>::schemas(&mut schemas);
    <CreateProductDto as ToSchema>::schemas(&mut schemas);
    <UpdateProductDto as ToSchema>::schemas(&mut schemas);
    <ProductResponse as ToSchema>::schemas(&mut schemas);
    <AddItemDto as ToSchema>::schemas(&mut schemas);
    <UpdateQtyDto as ToSchema>::schemas(&mut schemas);
    <CheckoutDto as ToSchema>::schemas(&mut schemas);
    for (name, schema) in schemas {
        cb = cb.schema(name, schema);
    }

    let extra = cb.build();

    let comps = api.components.get_or_insert_with(Default::default);
    comps.schemas.extend(extra.schemas);

    let tags = vec![
        TagBuilder::new().name("Auth").description(Some("Authentication: register, login, JWT token exchange.")).build(),
        TagBuilder::new().name("Users").description(Some("User management (admin listing + profile).")).build(),
        TagBuilder::new().name("Categories").description(Some("Product category CRUD.")).build(),
        TagBuilder::new().name("Products").description(Some("Product catalog CRUD + search.")).build(),
        TagBuilder::new().name("Cart").description(Some("Shopping cart — protected routes.")).build(),
        TagBuilder::new().name("Orders").description(Some("Checkout and order history — protected routes.")).build(),
        TagBuilder::new().name("Health").description(Some("Application liveness checks.")).build(),
        TagBuilder::new().name("Swagger").description(Some("OpenAPI document + Swagger UI.")).build(),
    ];
    api.tags = Some(tags);

    fn assign_tag(path: &str) -> Option<&'static str> {
        Some(if path.starts_with("/auth") {
            "Auth"
        } else if path.starts_with("/users") {
            "Users"
        } else if path.starts_with("/categories") {
            "Categories"
        } else if path.starts_with("/products") {
            "Products"
        } else if path.starts_with("/cart") {
            "Cart"
        } else if path.starts_with("/orders") {
            "Orders"
        } else if path == "/healthz" || path == "/health" {
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
