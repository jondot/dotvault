mod provider;
pub use provider::*;

#[cfg(feature = "env")]
pub mod env;
#[cfg(feature = "env")]
pub use env::EnvResolver;

#[cfg(feature = "onepassword")]
pub mod onepassword;
#[cfg(feature = "onepassword")]
pub use onepassword::OnePasswordResolver;

#[cfg(feature = "keychain")]
pub mod keychain;
#[cfg(feature = "keychain")]
pub use keychain::KeychainResolver;

#[cfg(feature = "age-provider")]
pub mod age;
#[cfg(feature = "age-provider")]
pub use age::AgeResolver;

#[cfg(feature = "aws")]
pub mod aws;
#[cfg(feature = "aws")]
pub use aws::AwsResolver;

#[cfg(feature = "hashicorp")]
pub mod hashicorp;
#[cfg(feature = "hashicorp")]
pub use hashicorp::HashiCorpResolver;

#[cfg(feature = "gcp")]
pub mod gcp;
#[cfg(feature = "gcp")]
pub use gcp::GcpResolver;

#[cfg(feature = "keyzero")]
pub mod keyzero;
