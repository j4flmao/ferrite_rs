use tonic::server::NamedService;

pub const ORDERS_GRPC_SERVICE_NAME: &str = "ferrite.micro.orders.Orders";

#[derive(Debug, Default, Clone)]
pub struct OrdersGrpcService {
    pub service_name: &'static str,
}

impl OrdersGrpcService {
    pub fn new() -> Self {
        Self {
            service_name: ORDERS_GRPC_SERVICE_NAME,
        }
    }
}

impl NamedService for OrdersGrpcService {
    const NAME: &'static str = ORDERS_GRPC_SERVICE_NAME;
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct OrdersGrpcInfo {
    #[schema(example = "ferrite.micro.orders.Orders")]
    pub service: String,
    #[schema(example = "0.0.0.0:50054")]
    pub bind: String,
    #[schema(example = json!(["GetOrder","ListOrders","CreateOrder","UpdateStatus","CancelOrder"]))]
    pub methods: Vec<String>,
}

pub const ORDERS_GRPC_METHODS: &[&str] = &[
    "GetOrder",
    "ListOrders",
    "CreateOrder",
    "UpdateStatus",
    "CancelOrder",
];
