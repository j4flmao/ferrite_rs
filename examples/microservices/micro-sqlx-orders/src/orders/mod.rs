pub mod controller;
pub mod dto;
pub mod grpc;
pub mod handlers;
pub mod repo;
pub mod service;

pub use grpc::{OrdersGrpcService, ORDERS_GRPC_SERVICE_NAME};
