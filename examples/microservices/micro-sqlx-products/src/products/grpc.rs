use tonic::server::NamedService;

pub const PRODUCTS_GRPC_SERVICE_NAME: &str = "ferrite.micro.products.Products";

/// A lightweight tonic-registered gRPC service used as a demo for the
/// `ferrite-grpc` integration. In a real microservice this struct would
/// `#[tonic::async_trait]` impl the auto-generated `products_server::Products`
/// trait from `tonic-build` + a `.proto`. We use a plain `NamedService` here
/// so the demo compiles without a `build.rs` / protoc dependency — the
/// inventory + manual `GrpcServer::add_service` flow is exercised the same.
#[derive(Debug, Default, Clone)]
pub struct ProductsGrpcService {
    pub service_name: &'static str,
}

impl ProductsGrpcService {
    pub fn new() -> Self {
        Self {
            service_name: PRODUCTS_GRPC_SERVICE_NAME,
        }
    }
}

impl NamedService for ProductsGrpcService {
    const NAME: &'static str = PRODUCTS_GRPC_SERVICE_NAME;
}

/// gRPC response DTO mirror (we skip actual tonic RPC impl in the demo so it
/// stays buildable without `protoc` installed — the DI registration and port
/// binding both execute the same real code paths regardless).
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ProductsGrpcInfo {
    #[schema(example = "ferrite.micro.products.Products")]
    pub service: String,
    #[schema(example = "0.0.0.0:50051")]
    pub bind: String,
    #[schema(example = json!(["GetProduct","ListProducts","CreateProduct","AdjustStock"]))]
    pub methods: Vec<String>,
}

pub const PRODUCTS_GRPC_METHODS: &[&str] =
    &["GetProduct", "ListProducts", "CreateProduct", "AdjustStock"];
