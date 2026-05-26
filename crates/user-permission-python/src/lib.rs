// The `pyo3-async-runtimes::tokio::future_into_py` wrapper expands to
// `.into()` calls that produce identity `PyErr -> PyErr` conversions, which
// clippy flags. The lint is meaningless for generated code.
#![allow(clippy::useless_conversion, clippy::too_many_arguments)]

use pyo3::prelude::*;

mod database;
mod error;
mod group;
mod password;
mod server;
mod service_client;
mod token;
mod user;

/// Native Rust extension for the `user-permission` Python package.
///
/// Re-exported by `python/user_permission/__init__.py`. Build with `maturin build`.
#[pymodule]
fn _user_permission(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<database::PyDatabase>()?;
    m.add_class::<user::PyUser>()?;
    m.add_class::<user::PyUserManager>()?;
    m.add_class::<group::PyGroup>()?;
    m.add_class::<group::PyGroupManager>()?;
    m.add_class::<token::PyTokenManager>()?;
    m.add_class::<service_client::PyServiceClient>()?;
    m.add_class::<service_client::PyServiceClientManager>()?;

    m.add_function(wrap_pyfunction!(password::hash_password, m)?)?;
    m.add_function(wrap_pyfunction!(password::verify_password, m)?)?;
    m.add_function(wrap_pyfunction!(password::load_or_create_secret, m)?)?;
    m.add_function(wrap_pyfunction!(service_client::validate_scopes, m)?)?;
    m.add_function(wrap_pyfunction!(server::serve, m)?)?;

    m.add("SCOPE_USERS_READ", user_permission_core::SCOPE_USERS_READ)?;
    m.add("SCOPE_GROUPS_READ", user_permission_core::SCOPE_GROUPS_READ)?;
    m.add("ALL_SCOPES", user_permission_core::ALL_SCOPES.to_vec())?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
