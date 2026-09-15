use tonic::server::NamedService;
use utoipa::ToSchema;

pub const AUTH_GRPC_SERVICE_NAME: &str = "ferrite.micro.auth.Auth";

#[derive(Debug, Default, Clone)]
pub struct AuthGrpcService {
    pub service_name: &'static str,
}

impl AuthGrpcService {
    pub fn new() -> Self {
        Self {
            service_name: AUTH_GRPC_SERVICE_NAME,
        }
    }
}

impl NamedService for AuthGrpcService {
    const NAME: &'static str = AUTH_GRPC_SERVICE_NAME;
}

ferrite_grpc::submit_grpc_service!(AuthGrpcService);

#[derive(Debug, Clone, serde::Serialize, ToSchema)]
pub struct AuthGrpcInfo {
    #[schema(example = "ferrite.micro.auth.Auth")]
    pub service: String,
    #[schema(example = "0.0.0.0:50052")]
    pub bind: String,
    #[schema(example = json!(["ValidateToken","GetUser","Register","Login"]))]
    pub methods: Vec<String>,
}

pub const AUTH_GRPC_METHODS: &[&str] = &["ValidateToken", "GetUser", "Register", "Login"];
