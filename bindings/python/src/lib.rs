//! PyO3 bindings for the Rust MCD core parser and exporter.

use std::path::PathBuf;

use mcd_core::{
    McdError, McdPackage,
    annotations::AnnotationMetadata,
    document::{DocumentBlock, McdDocument},
    errors::{Diagnostic, DiagnosticLevel},
    export::{
        ChartExportItem, agent_context_export, annotation_export, chart_export,
        expanded_markdown_export, image_export, original_markdown_export, table_export,
    },
    images::ImageMetadata,
    pdf::{PdfConversionOptions, pdf_to_mcd_bytes as core_pdf_to_mcd_bytes},
    schema::TableSchema as CoreTableSchema,
    table_view::TableView as CoreTableView,
    tables::{DataTable, TableRow, TypedValue},
    validate,
};
use mcd_query::{
    QueryResult as CoreQueryResult, QueryValue as CoreQueryValue, query_package, query_path,
};
use pyo3::{
    IntoPyObjectExt,
    exceptions::{PyKeyError, PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyBytes, PyDict, PyList},
};
use serde_json::{Map, Value};

#[pyclass(name = "Document", module = "mcd")]
#[derive(Clone)]
struct PyDocument {
    path: PathBuf,
    package: McdPackage,
}

#[pymethods]
impl PyDocument {
    #[getter]
    fn path(&self) -> String {
        self.path.display().to_string()
    }

    fn validate(&self) -> PyResult<PyValidationResult> {
        match validate::validate_package(&self.package) {
            Ok(result) => Ok(PyValidationResult::from_core(result)),
            Err(err) => {
                if let Some(diagnostic) = err.diagnostic() {
                    Ok(PyValidationResult {
                        valid: false,
                        diagnostics: vec![PyDiagnostic::from_core(diagnostic)],
                    })
                } else {
                    Err(err_to_py(err))
                }
            }
        }
    }

    fn blocks(&self) -> PyResult<Vec<PyBlock>> {
        let manifest = self.package.manifest().map_err(err_to_py)?;
        let document = McdDocument::from_package(&self.package, &manifest).map_err(err_to_py)?;
        Ok(document
            .blocks
            .into_iter()
            .map(PyBlock::from_core)
            .collect())
    }

    fn table(&self, id: &str) -> PyResult<PyTable> {
        let tables = table_export(&self.package).map_err(err_to_py)?.tables;
        tables
            .into_iter()
            .find(|table| table.id == id)
            .map(PyTable::new)
            .ok_or_else(|| PyKeyError::new_err(format!("unknown table '{id}'")))
    }

    fn chart(&self, id: &str) -> PyResult<PyChart> {
        let charts = chart_export(&self.package).map_err(err_to_py)?.charts;
        let tables = table_export(&self.package).map_err(err_to_py)?.tables;
        let chart = charts
            .into_iter()
            .find(|chart| {
                chart.block_id == id
                    || chart.view_id == id
                    || chart.placement_ref.as_deref() == Some(id)
            })
            .ok_or_else(|| PyKeyError::new_err(format!("unknown chart '{id}'")))?;
        let table = tables
            .into_iter()
            .find(|table| table.id == chart.table_id)
            .ok_or_else(|| {
                PyKeyError::new_err(format!("unknown chart table '{}'", chart.table_id))
            })?;
        Ok(PyChart { chart, table })
    }

    fn image(&self, id: &str) -> PyResult<PyImage> {
        let images = image_export(&self.package).map_err(err_to_py)?.images;
        images
            .into_iter()
            .find(|image| {
                image.id == id
                    || image.asset == id
                    || image.asset.strip_prefix("assets/") == Some(id)
            })
            .map(|image| PyImage { image })
            .ok_or_else(|| PyKeyError::new_err(format!("unknown image '{id}'")))
    }

    fn annotation(&self, id: &str) -> PyResult<PyAnnotation> {
        let annotations = annotation_export(&self.package)
            .map_err(err_to_py)?
            .annotations;
        annotations
            .into_iter()
            .find(|annotation| annotation.id == id)
            .map(|annotation| PyAnnotation { annotation })
            .ok_or_else(|| PyKeyError::new_err(format!("unknown annotation '{id}'")))
    }

    fn annotations(&self) -> PyResult<Vec<PyAnnotation>> {
        Ok(annotation_export(&self.package)
            .map_err(err_to_py)?
            .annotations
            .into_iter()
            .map(|annotation| PyAnnotation { annotation })
            .collect())
    }

    #[pyo3(signature = (expand_tables = false))]
    fn markdown(&self, expand_tables: bool) -> PyResult<String> {
        if expand_tables {
            expanded_markdown_export(&self.package).map_err(err_to_py)
        } else {
            original_markdown_export(&self.package).map_err(err_to_py)
        }
    }

    #[pyo3(signature = (include_tables = true, include_layout = false))]
    fn to_agent_context(
        &self,
        py: Python<'_>,
        include_tables: bool,
        include_layout: bool,
    ) -> PyResult<PyObject> {
        let _ = include_layout;
        let mut value =
            serde_json::to_value(agent_context_export(&self.package).map_err(err_to_py)?)
                .map_err(json_err_to_py)?;
        if !include_tables && let Value::Object(object) = &mut value {
            object.remove("tables");
        }
        json_to_py(py, &value)
    }

    fn query(&self, sql: &str) -> PyResult<PyQueryResult> {
        Ok(PyQueryResult {
            result: query_package(&self.package, sql).map_err(query_err_to_py)?,
        })
    }

    fn __repr__(&self) -> String {
        format!("Document(path={:?})", self.path.display().to_string())
    }
}

#[pyclass(name = "Block", module = "mcd")]
#[derive(Clone)]
struct PyBlock {
    block: DocumentBlock,
    value: Value,
}

impl PyBlock {
    fn from_core(block: DocumentBlock) -> Self {
        let value = serde_json::to_value(&block).unwrap_or(Value::Null);
        Self { block, value }
    }
}

#[pymethods]
impl PyBlock {
    #[getter]
    fn id(&self) -> String {
        self.block.id().to_owned()
    }

    #[getter]
    fn r#type(&self) -> PyResult<String> {
        self.value
            .get("type")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| PyRuntimeError::new_err("block type is missing"))
    }

    #[getter]
    fn source(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(py, self.value.get("source").unwrap_or(&Value::Null))
    }

    fn as_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(py, &self.value)
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Block(id={:?}, type={:?})",
            self.id(),
            self.r#type()?
        ))
    }
}

#[pyclass(name = "Table", module = "mcd")]
#[derive(Clone)]
struct PyTable {
    table: DataTable,
}

impl PyTable {
    fn new(table: DataTable) -> Self {
        Self { table }
    }
}

#[pymethods]
impl PyTable {
    #[getter]
    fn id(&self) -> String {
        self.table.id.clone()
    }

    #[getter]
    fn source(&self) -> String {
        self.table.source.clone()
    }

    #[getter]
    fn schema(&self) -> PyTableSchema {
        PyTableSchema {
            schema: self.table.schema.clone(),
        }
    }

    fn rows(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(py, &plain_rows_json(&self.table.rows))
    }

    fn typed_rows(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.table.rows).map_err(json_err_to_py)?,
        )
    }

    fn dataframe(&self, py: Python<'_>) -> PyResult<PyObject> {
        dataframe_from_rows(py, &self.table.rows)
    }

    fn as_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.table).map_err(json_err_to_py)?,
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "Table(id={:?}, rows={})",
            self.table.id,
            self.table.rows.len()
        )
    }
}

#[pyclass(name = "TableSchema", module = "mcd")]
#[derive(Clone)]
struct PyTableSchema {
    schema: CoreTableSchema,
}

#[pymethods]
impl PyTableSchema {
    #[getter]
    fn id(&self) -> String {
        self.schema.id.clone()
    }

    #[getter]
    fn columns(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.schema.columns).map_err(json_err_to_py)?,
        )
    }

    #[getter]
    fn primary_key(&self) -> Vec<String> {
        self.schema.primary_key.clone()
    }

    #[getter]
    fn foreign_keys(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.schema.foreign_keys).map_err(json_err_to_py)?,
        )
    }

    fn as_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.schema).map_err(json_err_to_py)?,
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "TableSchema(id={:?}, columns={})",
            self.schema.id,
            self.schema.columns.len()
        )
    }
}

#[pyclass(name = "TableView", module = "mcd")]
#[derive(Clone)]
struct PyTableView {
    view: CoreTableView,
}

#[pymethods]
impl PyTableView {
    #[getter]
    fn id(&self) -> String {
        self.view.id.clone()
    }

    #[getter]
    fn table_id(&self) -> String {
        self.view.table.clone()
    }

    #[getter]
    fn display(&self) -> String {
        format!("{:?}", self.view.display).to_ascii_lowercase()
    }

    #[getter]
    fn columns(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.view.columns).map_err(json_err_to_py)?,
        )
    }

    #[getter]
    fn chart(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.view.chart).map_err(json_err_to_py)?,
        )
    }

    fn layout(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(py, self.view.style.as_ref().unwrap_or(&Value::Null))
    }

    fn as_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.view).map_err(json_err_to_py)?,
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "TableView(id={:?}, table_id={:?}, display={:?})",
            self.view.id,
            self.view.table,
            self.display()
        )
    }
}

#[pyclass(name = "Chart", module = "mcd")]
#[derive(Clone)]
struct PyChart {
    chart: ChartExportItem,
    table: DataTable,
}

#[pymethods]
impl PyChart {
    #[getter]
    fn table_id(&self) -> String {
        self.chart.table_id.clone()
    }

    #[getter]
    fn view_id(&self) -> String {
        self.chart.view_id.clone()
    }

    #[getter]
    fn placement_ref(&self) -> Option<String> {
        self.chart.placement_ref.clone()
    }

    #[getter]
    fn view(&self) -> PyTableView {
        PyTableView {
            view: self.chart.view.clone(),
        }
    }

    fn rows(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(py, &plain_rows_json(&self.chart.rows))
    }

    fn dataframe(&self, py: Python<'_>) -> PyResult<PyObject> {
        dataframe_from_rows(py, &self.chart.rows)
    }

    fn to_markdown_table(&self) -> String {
        markdown_table_for_chart(&self.chart, &self.table)
    }

    fn layout(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(py, self.chart.view.style.as_ref().unwrap_or(&Value::Null))
    }

    fn as_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.chart).map_err(json_err_to_py)?,
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "Chart(table_id={:?}, view_id={:?})",
            self.chart.table_id, self.chart.view_id
        )
    }
}

#[pyclass(name = "Image", module = "mcd")]
#[derive(Clone)]
struct PyImage {
    image: ImageMetadata,
}

#[pymethods]
impl PyImage {
    #[getter]
    fn id(&self) -> String {
        self.image.id.clone()
    }

    #[getter]
    fn asset_path(&self) -> String {
        self.image.asset.clone()
    }

    #[getter]
    fn role(&self) -> String {
        format!("{:?}", self.image.role).to_kebab_case()
    }

    #[getter]
    fn alt(&self) -> Option<String> {
        self.image.alt.clone()
    }

    #[getter]
    fn caption(&self) -> Option<String> {
        self.image.caption.clone()
    }

    #[getter]
    fn intrinsic_size(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.image.intrinsic_size).map_err(json_err_to_py)?,
        )
    }

    fn as_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.image).map_err(json_err_to_py)?,
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "Image(id={:?}, asset_path={:?})",
            self.image.id, self.image.asset
        )
    }
}

#[pyclass(name = "Annotation", module = "mcd")]
#[derive(Clone)]
struct PyAnnotation {
    annotation: AnnotationMetadata,
}

#[pymethods]
impl PyAnnotation {
    #[getter]
    fn id(&self) -> String {
        self.annotation.id.clone()
    }

    #[getter]
    fn kind(&self) -> String {
        format!("{:?}", self.annotation.kind).to_snake_case()
    }

    #[getter]
    fn status(&self) -> String {
        format!("{:?}", self.annotation.status).to_snake_case()
    }

    #[getter]
    fn body(&self) -> String {
        self.annotation.body.clone()
    }

    #[getter]
    fn labels(&self) -> Vec<String> {
        self.annotation.labels.clone()
    }

    fn target(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.annotation.target).map_err(json_err_to_py)?,
        )
    }

    fn proposed_change(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.annotation.proposed_change).map_err(json_err_to_py)?,
        )
    }

    fn as_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(
            py,
            &serde_json::to_value(&self.annotation).map_err(json_err_to_py)?,
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "Annotation(id={:?}, kind={:?}, status={:?})",
            self.annotation.id,
            self.kind(),
            self.status()
        )
    }
}

#[pyclass(name = "QueryResult", module = "mcd")]
#[derive(Clone)]
struct PyQueryResult {
    result: CoreQueryResult,
}

#[pymethods]
impl PyQueryResult {
    #[getter]
    fn columns(&self) -> Vec<String> {
        self.result.columns.clone()
    }

    #[getter]
    fn row_count(&self) -> usize {
        self.result.row_count()
    }

    #[getter]
    fn rows(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(py, &self.result.rows_as_json())
    }

    fn values(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(py, &query_values_json(&self.result))
    }

    fn as_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(py, &self.result.as_json())
    }

    fn to_json(&self) -> PyResult<String> {
        self.result.to_json_pretty().map_err(query_err_to_py)
    }

    fn to_csv(&self) -> String {
        self.result.to_csv()
    }

    fn to_table(&self) -> String {
        self.result.to_table()
    }

    fn __len__(&self) -> usize {
        self.result.row_count()
    }

    fn __repr__(&self) -> String {
        format!(
            "QueryResult(columns={}, rows={})",
            self.result.columns.len(),
            self.result.row_count()
        )
    }
}

#[pyclass(name = "ValidationResult", module = "mcd")]
#[derive(Clone)]
struct PyValidationResult {
    #[pyo3(get)]
    valid: bool,
    #[pyo3(get)]
    diagnostics: Vec<PyDiagnostic>,
}

impl PyValidationResult {
    fn from_core(result: mcd_core::ValidationResult) -> Self {
        Self {
            valid: result.valid,
            diagnostics: result
                .diagnostics
                .iter()
                .map(PyDiagnostic::from_core)
                .collect(),
        }
    }
}

#[pymethods]
impl PyValidationResult {
    fn as_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let diagnostics = self
            .diagnostics
            .iter()
            .map(PyDiagnostic::to_json)
            .collect::<Vec<_>>();
        json_to_py(
            py,
            &serde_json::json!({
                "valid": self.valid,
                "diagnostics": diagnostics,
            }),
        )
    }

    fn __bool__(&self) -> bool {
        self.valid
    }

    fn __repr__(&self) -> String {
        format!(
            "ValidationResult(valid={}, diagnostics={})",
            self.valid,
            self.diagnostics.len()
        )
    }
}

#[pyclass(name = "Diagnostic", module = "mcd")]
#[derive(Clone)]
struct PyDiagnostic {
    #[pyo3(get)]
    level: String,
    #[pyo3(get)]
    code: String,
    #[pyo3(get)]
    message: String,
    #[pyo3(get)]
    source: Option<String>,
    #[pyo3(get)]
    related: Vec<String>,
}

impl PyDiagnostic {
    fn from_core(diagnostic: &Diagnostic) -> Self {
        Self {
            level: match diagnostic.level {
                DiagnosticLevel::Error => "error",
                DiagnosticLevel::Warning => "warning",
                DiagnosticLevel::Info => "info",
            }
            .to_owned(),
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            source: diagnostic.source.clone(),
            related: diagnostic.related.clone(),
        }
    }

    fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert("level".to_owned(), Value::String(self.level.clone()));
        object.insert("code".to_owned(), Value::String(self.code.clone()));
        object.insert("message".to_owned(), Value::String(self.message.clone()));
        if let Some(source) = &self.source {
            object.insert("source".to_owned(), Value::String(source.clone()));
        }
        if !self.related.is_empty() {
            object.insert(
                "related".to_owned(),
                Value::Array(
                    self.related
                        .iter()
                        .map(|value| Value::String(value.clone()))
                        .collect(),
                ),
            );
        }
        Value::Object(object)
    }
}

#[pymethods]
impl PyDiagnostic {
    fn as_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(py, &self.to_json())
    }

    fn __repr__(&self) -> String {
        format!("Diagnostic(level={:?}, code={:?})", self.level, self.code)
    }
}

#[pyfunction(name = "open")]
fn open_package(path: PathBuf) -> PyResult<PyDocument> {
    let package = McdPackage::open_path(&path).map_err(err_to_py)?;
    Ok(PyDocument { path, package })
}

#[pyfunction]
#[pyo3(signature = (input, output, title = None))]
fn convert_pdf(input: PathBuf, output: PathBuf, title: Option<String>) -> PyResult<PyDocument> {
    let pdf = std::fs::read(&input)
        .map_err(McdError::from)
        .map_err(err_to_py)?;
    let bytes = core_pdf_to_mcd_bytes(
        &pdf,
        PdfConversionOptions {
            title,
            source_filename: input
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned),
        },
    )
    .map_err(err_to_py)?;
    std::fs::write(&output, bytes)
        .map_err(McdError::from)
        .map_err(err_to_py)?;
    let package = McdPackage::open_path(&output).map_err(err_to_py)?;
    Ok(PyDocument {
        path: output,
        package,
    })
}

#[pyfunction]
#[pyo3(signature = (pdf, title = None, source_filename = None))]
fn pdf_to_mcd_bytes<'py>(
    py: Python<'py>,
    pdf: &[u8],
    title: Option<String>,
    source_filename: Option<String>,
) -> PyResult<Bound<'py, PyBytes>> {
    let bytes = core_pdf_to_mcd_bytes(
        pdf,
        PdfConversionOptions {
            title,
            source_filename,
        },
    )
    .map_err(err_to_py)?;
    Ok(PyBytes::new(py, &bytes))
}

#[pyfunction(name = "query")]
fn query_file(path: PathBuf, sql: &str) -> PyResult<PyQueryResult> {
    Ok(PyQueryResult {
        result: query_path(path, sql).map_err(query_err_to_py)?,
    })
}

#[pymodule]
fn _native(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(open_package, module)?)?;
    module.add_function(wrap_pyfunction!(convert_pdf, module)?)?;
    module.add_function(wrap_pyfunction!(pdf_to_mcd_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(query_file, module)?)?;
    module.add_class::<PyDocument>()?;
    module.add_class::<PyBlock>()?;
    module.add_class::<PyTable>()?;
    module.add_class::<PyTableSchema>()?;
    module.add_class::<PyTableView>()?;
    module.add_class::<PyChart>()?;
    module.add_class::<PyImage>()?;
    module.add_class::<PyAnnotation>()?;
    module.add_class::<PyQueryResult>()?;
    module.add_class::<PyValidationResult>()?;
    module.add_class::<PyDiagnostic>()?;
    Ok(())
}

fn err_to_py(err: McdError) -> PyErr {
    if let Some(diagnostic) = err.diagnostic() {
        PyValueError::new_err(format!("{}: {}", diagnostic.code, diagnostic.message))
    } else {
        PyRuntimeError::new_err(err.to_string())
    }
}

fn json_err_to_py(err: serde_json::Error) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

fn query_err_to_py(err: anyhow::Error) -> PyErr {
    PyValueError::new_err(err.to_string())
}

fn json_to_py(py: Python<'_>, value: &Value) -> PyResult<PyObject> {
    match value {
        Value::Null => Ok(py.None()),
        Value::Bool(value) => value.into_py_any(py),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                value.into_py_any(py)
            } else if let Some(value) = value.as_u64() {
                value.into_py_any(py)
            } else if let Some(value) = value.as_f64() {
                value.into_py_any(py)
            } else {
                Ok(py.None())
            }
        }
        Value::String(value) => value.into_py_any(py),
        Value::Array(values) => {
            let list = PyList::empty(py);
            for item in values {
                list.append(json_to_py(py, item)?)?;
            }
            list.into_py_any(py)
        }
        Value::Object(values) => {
            let dict = PyDict::new(py);
            for (key, value) in values {
                dict.set_item(key, json_to_py(py, value)?)?;
            }
            dict.into_py_any(py)
        }
    }
}

fn query_values_json(result: &CoreQueryResult) -> Value {
    Value::Array(
        result
            .rows
            .iter()
            .map(|row| Value::Array(row.iter().map(query_value_to_json).collect::<Vec<_>>()))
            .collect(),
    )
}

fn query_value_to_json(value: &CoreQueryValue) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn dataframe_from_rows(py: Python<'_>, rows: &[TableRow]) -> PyResult<PyObject> {
    let pandas = py.import("pandas").map_err(|_| {
        PyRuntimeError::new_err("pandas is required for dataframe(); install mcd[pandas]")
    })?;
    let rows = json_to_py(py, &plain_rows_json(rows))?;
    pandas.getattr("DataFrame")?.call1((rows,))?.into_py_any(py)
}

fn plain_rows_json(rows: &[TableRow]) -> Value {
    Value::Array(
        rows.iter()
            .map(|row| {
                let mut object = Map::new();
                for (name, value) in &row.cells {
                    object.insert(name.clone(), typed_value_to_json(value));
                }
                Value::Object(object)
            })
            .collect(),
    )
}

fn typed_value_to_json(value: &TypedValue) -> Value {
    match value {
        TypedValue::Null => Value::Null,
        TypedValue::String(value)
        | TypedValue::Decimal(value)
        | TypedValue::Date(value)
        | TypedValue::Datetime(value)
        | TypedValue::Time(value)
        | TypedValue::Enum(value) => Value::String(value.clone()),
        TypedValue::Integer(value) => Value::Number((*value).into()),
        TypedValue::Boolean(value) => Value::Bool(*value),
    }
}

fn markdown_table_for_chart(chart: &ChartExportItem, table: &DataTable) -> String {
    let columns = chart_column_names(chart, table);
    let headers = columns
        .iter()
        .map(|name| {
            table
                .schema
                .column(name)
                .and_then(|column| column.label.clone())
                .unwrap_or_else(|| name.clone())
        })
        .collect::<Vec<_>>();
    let mut lines = vec![
        format!("| {} |", headers.join(" | ")),
        format!(
            "| {} |",
            columns
                .iter()
                .map(|_| "---")
                .collect::<Vec<_>>()
                .join(" | ")
        ),
    ];

    for row in &chart.rows {
        let cells = columns
            .iter()
            .map(|name| {
                row.cells
                    .get(name)
                    .map(typed_value_to_markdown)
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        lines.push(format!("| {} |", cells.join(" | ")));
    }
    lines.join("\n")
}

fn chart_column_names(chart: &ChartExportItem, table: &DataTable) -> Vec<String> {
    let Some(spec) = &chart.view.chart else {
        return table
            .schema
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect();
    };

    let mut columns = Vec::new();
    push_unique(&mut columns, &spec.x.column);
    push_unique(&mut columns, &spec.y.column);
    if let Some(series) = &spec.series {
        push_unique(&mut columns, &series.column);
    }
    if let Some(grouping) = &spec.grouping {
        push_unique(&mut columns, &grouping.column);
    }
    if let Some(mark_labels) = &spec.mark_labels
        && let Some(column) = &mark_labels.column
    {
        push_unique(&mut columns, column);
    }
    columns
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_owned());
    }
}

fn typed_value_to_markdown(value: &TypedValue) -> String {
    match typed_value_to_json(value) {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.replace('\n', " ").replace('|', r"\|"),
        Value::Array(_) | Value::Object(_) => String::new(),
    }
}

trait ToKebabCase {
    fn to_kebab_case(&self) -> String;
}

impl ToKebabCase for str {
    fn to_kebab_case(&self) -> String {
        let mut output = String::new();
        for (index, character) in self.chars().enumerate() {
            if character.is_ascii_uppercase() {
                if index > 0 {
                    output.push('-');
                }
                output.push(character.to_ascii_lowercase());
            } else {
                output.push(character);
            }
        }
        output
    }
}

trait ToSnakeCase {
    fn to_snake_case(&self) -> String;
}

impl ToSnakeCase for str {
    fn to_snake_case(&self) -> String {
        let mut output = String::new();
        for (index, character) in self.chars().enumerate() {
            if character.is_ascii_uppercase() {
                if index > 0 {
                    output.push('_');
                }
                output.push(character.to_ascii_lowercase());
            } else {
                output.push(character);
            }
        }
        output
    }
}
