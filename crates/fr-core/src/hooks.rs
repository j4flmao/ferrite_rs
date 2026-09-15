//! Lifecycle hook traits, mirroring Nest's `OnModuleInit`,
//! `OnApplicationBootstrap`, `OnModuleDestroy`, `OnApplicationShutdown`.

use async_trait::async_trait;

/// Runs once the module's providers and controllers are instantiated.
#[async_trait]
pub trait OnModuleInit {
    async fn on_module_init(&self) {}
}

/// Runs after the full module graph has booted, before binding the transport.
#[async_trait]
pub trait OnApplicationBootstrap {
    async fn on_application_bootstrap(&self) {}
}

/// Runs on shutdown, in reverse module order.
#[async_trait]
pub trait OnModuleDestroy {
    async fn on_module_destroy(&self) {}
}

/// Runs last during graceful shutdown.
#[async_trait]
pub trait OnApplicationShutdown {
    async fn on_application_shutdown(&self) {}
}
