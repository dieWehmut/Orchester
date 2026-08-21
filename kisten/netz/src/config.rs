use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticAssets {
    Disabled,
    Directory(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerConfigError {
    NonLoopbackBind { ip: IpAddr },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    bind_ip: IpAddr,
    port: u16,
    static_assets: StaticAssets,
}

impl ServerConfig {
    pub fn new(bind_ip: IpAddr, port: u16) -> Self {
        Self {
            bind_ip,
            port,
            static_assets: StaticAssets::Disabled,
        }
    }

    pub fn bind_ip(&self) -> IpAddr {
        self.bind_ip
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.bind_ip, self.port)
    }

    pub fn validate_loopback(&self) -> Result<(), ServerConfigError> {
        if self.bind_ip.is_loopback() {
            Ok(())
        } else {
            Err(ServerConfigError::NonLoopbackBind { ip: self.bind_ip })
        }
    }

    pub fn static_assets(&self) -> &StaticAssets {
        &self.static_assets
    }

    pub fn with_static_assets(mut self, static_assets: StaticAssets) -> Self {
        self.static_assets = static_assets;
        self
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
    }
}
