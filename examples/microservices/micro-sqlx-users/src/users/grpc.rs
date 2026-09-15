use tonic::server::NamedService;

pub const USERS_GRPC_SERVICE_NAME: &str = "ferrite.micro.users.Users";

#[derive(Debug, Default, Clone)]
pub struct UsersGrpcService {
    pub service_name: &'static str,
}

impl UsersGrpcService {
    pub fn new() -> Self {
        Self {
            service_name: USERS_GRPC_SERVICE_NAME,
        }
    }
}

impl NamedService for UsersGrpcService {
    const NAME: &'static str = USERS_GRPC_SERVICE_NAME;
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct UsersGrpcInfo {
    #[schema(example = "ferrite.micro.users.Users")]
    pub service: String,
    #[schema(example = "0.0.0.0:50053")]
    pub bind: String,
    #[schema(example = json!(["GetUser","ListUsers","CreateUser","UpdateUser","DeleteUser","FindByEmail"]))]
    pub methods: Vec<String>,
}

pub const USERS_GRPC_METHODS: &[&str] = &[
    "GetUser",
    "ListUsers",
    "CreateUser",
    "UpdateUser",
    "DeleteUser",
    "FindByEmail",
];
