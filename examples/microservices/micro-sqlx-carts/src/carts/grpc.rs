use tonic::server::NamedService;
use utoipa::ToSchema;

pub const CARTS_GRPC_SERVICE_NAME: &str = "ferrite.micro.carts.Carts";

#[derive(Debug, Default, Clone)]
pub struct CartsGrpcService {
    pub service_name: &'static str,
}

impl CartsGrpcService {
    pub fn new() -> Self {
        Self {
            service_name: CARTS_GRPC_SERVICE_NAME,
        }
    }
}

impl NamedService for CartsGrpcService {
    const NAME: &'static str = CARTS_GRPC_SERVICE_NAME;
}

ferrite_grpc::submit_grpc_service!(CartsGrpcService);

#[derive(Debug, Clone, serde::Serialize, ToSchema)]
pub struct CartsGrpcInfo {
    #[schema(example = "ferrite.micro.carts.Carts")]
    pub service: String,
    #[schema(example = "0.0.0.0:50055")]
    pub bind: String,
    #[schema(example = json!(["GetCart","ListCarts","CreateCart","AddItem","UpdateItem","RemoveItem","ClearCart"]))]
    pub methods: Vec<String>,
}

pub const CARTS_GRPC_METHODS: &[&str] = &[
    "GetCart",
    "ListCarts",
    "CreateCart",
    "AddItem",
    "UpdateItem",
    "RemoveItem",
    "ClearCart",
];
