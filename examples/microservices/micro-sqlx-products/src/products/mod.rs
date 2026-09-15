pub mod controllers;
pub mod dto;
pub mod grpc;
pub mod handlers;
pub mod repo;

pub use grpc::{ProductsGrpcService, PRODUCTS_GRPC_SERVICE_NAME};
