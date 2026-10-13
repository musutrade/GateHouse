//! Typed v1 wire models. Construction from untrusted JSON must use the validators.
use serde::{Deserialize, Serialize};
use serde_json::Number;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub id: String,
    pub boundaries: Vec<String>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBoundary {
    pub id: String,
    pub path: String,
    pub role: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub path: String,
    pub metadata: BTreeMap<String, String>,
    pub targets: Vec<Target>,
    pub source_boundaries: Vec<SourceBoundary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    pub start_line: Number,
    pub start_column: Number,
    pub end_line: Number,
    pub end_column: Number,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    pub id: String,
    pub identity_version: String,
    pub component: String,
    pub target: String,
    pub boundary: String,
    pub kind: String,
    pub path: String,
    pub discriminator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    pub source_sha256: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub id: String,
    pub kind: String,
    pub producer: String,
    pub consumer: String,
    pub subjects: Vec<String>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema: String,
    pub id: String,
    pub metadata: BTreeMap<String, String>,
    pub components: Vec<Component>,
    pub subjects: Vec<Subject>,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Versioned {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub commit: String,
    pub base_commit: String,
    pub target: String,
    pub run: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricContract {
    pub name: String,
    pub r#type: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series {
    pub id: String,
    pub name: String,
    pub collector: Versioned,
    pub tool: Versioned,
    pub rule: Versioned,
    pub runtime: Versioned,
    pub target: String,
    pub source_identity: Versioned,
    pub normalization: Versioned,
    pub metrics: Vec<MetricContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    pub name: String,
    pub value: MetricValue,
    pub artifacts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub metric: String,
    pub state: CapabilityState,
    pub reason: String,
    pub artifacts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub id: String,
    pub kind: String,
    pub media_type: String,
    pub path: String,
    pub sha256: String,
    pub bytes: Number,
    pub context: Context,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub relationship: String,
    pub producer: String,
    pub consumer: String,
    pub contract_artifact: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<ContractBaseline>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_artifact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_client: Option<ContractGeneratedClient>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    pub schema: String,
    pub id: String,
    pub project: String,
    pub component: String,
    pub collector: Versioned,
    pub series: Series,
    pub subject: Subject,
    pub context: Context,
    pub source: Source,
    pub metrics: Vec<Metric>,
    pub capabilities: Vec<Capability>,
    pub artifacts: Vec<Artifact>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<Contract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractBaseline {
    pub artifact: String,
    pub commit: String,
    pub series_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractGeneratedClient {
    pub artifact: String,
    pub contract_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Supported,
    Unsupported,
    NotConfigured,
    NotCollected,
    MeasurementError,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    Ratio,
    Count,
    Boolean,
    Duration,
    Size,
    Decimal,
    Rational,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MetricValue {
    Ratio {
        covered: Number,
        total: Number,
    },
    Count {
        value: Number,
    },
    Boolean {
        value: bool,
    },
    Duration {
        value: Number,
        unit: String,
    },
    Size {
        value: Number,
        unit: String,
    },
    Decimal {
        value: String,
    },
    Rational {
        numerator: Number,
        denominator: Number,
    },
}
