use std::net::TcpListener;

use crate::{ServerConfig, ServerConfigError};

#[derive(Debug)]
pub enum ServerBindError {
    InvalidConfig(ServerConfigError),
    BindFailed(std::io::Error),
}

pub fn bind_listener(config: &ServerConfig) -> Result<TcpListener, ServerBindError> {
    config
        .validate_loopback()
        .map_err(ServerBindError::InvalidConfig)?;

    TcpListener::bind(config.socket_addr()).map_err(ServerBindError::BindFailed)
}
