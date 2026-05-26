use std::time::Duration;

use jsonwebtoken::Algorithm;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use user_permission_core::TokenManager;

use crate::error::map_core_err;

#[pyclass(module = "user_permission", name = "TokenManager")]
pub struct PyTokenManager {
    inner: TokenManager,
}

impl PyTokenManager {
    pub(crate) fn from_inner(inner: TokenManager) -> Self {
        Self { inner }
    }
}

fn algorithm_from_str(name: &str) -> PyResult<Algorithm> {
    match name {
        "HS256" => Ok(Algorithm::HS256),
        "HS384" => Ok(Algorithm::HS384),
        "HS512" => Ok(Algorithm::HS512),
        other => Err(PyValueError::new_err(format!(
            "unsupported algorithm: {other}"
        ))),
    }
}

fn pydelta_to_duration(delta: &Bound<'_, PyAny>) -> PyResult<Duration> {
    let total_seconds: f64 = delta.call_method0("total_seconds")?.extract()?;
    if total_seconds <= 0.0 {
        return Ok(Duration::from_secs(0));
    }
    Ok(Duration::from_secs_f64(total_seconds))
}

fn pyobject_to_json(value: &Bound<'_, PyAny>) -> PyResult<Value> {
    let py = value.py();
    let json_module = py.import_bound("json")?;
    let s: String = json_module
        .call_method1("dumps", (value,))?
        .extract()?;
    serde_json::from_str(&s).map_err(|e| PyValueError::new_err(e.to_string()))
}

fn json_to_pyobject<'py>(py: Python<'py>, value: &Value) -> PyResult<Bound<'py, PyAny>> {
    let json_module = py.import_bound("json")?;
    let s = serde_json::to_string(value).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    json_module.call_method1("loads", (s,))
}

#[pymethods]
impl PyTokenManager {
    #[new]
    #[pyo3(signature = (secret, algorithm="HS256"))]
    fn new(secret: &str, algorithm: &str) -> PyResult<Self> {
        Ok(Self {
            inner: TokenManager::new(secret, algorithm_from_str(algorithm)?),
        })
    }

    #[classmethod]
    #[pyo3(signature = (path, algorithm="HS256"))]
    fn from_file(
        _cls: &Bound<'_, pyo3::types::PyType>,
        path: &str,
        algorithm: &str,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: TokenManager::from_file(path, algorithm_from_str(algorithm)?)
                .map_err(map_core_err)?,
        })
    }

    #[pyo3(signature = (user_id, username, expires_delta=None, extra_claims=None))]
    fn create_token(
        &self,
        py: Python<'_>,
        user_id: i64,
        username: &str,
        expires_delta: Option<&Bound<'_, PyAny>>,
        extra_claims: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<String> {
        let duration = match expires_delta {
            Some(d) => pydelta_to_duration(d)?,
            None => Duration::from_secs(3600),
        };
        let extra = match extra_claims {
            Some(d) => {
                let v = pyobject_to_json(d.as_any())?;
                match v {
                    Value::Object(map) => Some(map),
                    _ => None,
                }
            }
            None => None,
        };
        let _ = py; // currently unused; reserved for future GIL-bound work
        self.inner
            .create_token(user_id, username, duration, extra.as_ref())
            .map_err(map_core_err)
    }

    fn verify_token<'py>(
        &self,
        py: Python<'py>,
        token: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let claims = self.inner.verify_token(token).map_err(map_core_err)?;
        json_to_pyobject(py, &Value::Object(claims))
    }

    #[pyo3(signature = (client_id, scopes, expires_delta=None))]
    fn create_service_token(
        &self,
        client_id: &str,
        scopes: Vec<String>,
        expires_delta: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<String> {
        let duration = match expires_delta {
            Some(d) => pydelta_to_duration(d)?,
            None => Duration::from_secs(3600),
        };
        self.inner
            .create_service_token(client_id, &scopes, duration)
            .map_err(map_core_err)
    }
}
