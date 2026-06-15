//! Hand-written CDP command types for the subset of commands nissia needs.
//! We intentionally avoid generated CDP bindings to keep binary size small
//! and compile times fast.

use serde::{Deserialize, Serialize};

use crate::types::{BackendNodeId, BoxModel, CdpCommand, NodeId, RemoteObject};

// ============================================================
// Target domain
// ============================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetCreateTarget {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetCreateTargetResponse {
    pub target_id: String,
}

impl CdpCommand for TargetCreateTarget {
    type Response = TargetCreateTargetResponse;
    fn method_name(&self) -> &'static str {
        "Target.createTarget"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetAttachToTarget {
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flatten: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetAttachToTargetResponse {
    pub session_id: String,
}

impl CdpCommand for TargetAttachToTarget {
    type Response = TargetAttachToTargetResponse;
    fn method_name(&self) -> &'static str {
        "Target.attachToTarget"
    }
}

#[derive(Debug, Serialize)]
pub struct TargetGetTargets {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetGetTargetsResponse {
    pub target_infos: Vec<TargetInfoItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfoItem {
    pub target_id: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub title: String,
    pub url: String,
    pub attached: bool,
}

impl CdpCommand for TargetGetTargets {
    type Response = TargetGetTargetsResponse;
    fn method_name(&self) -> &'static str {
        "Target.getTargets"
    }
}

// ============================================================
// Page domain
// ============================================================

#[derive(Debug, Serialize)]
pub struct PageEnable {}

impl CdpCommand for PageEnable {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Page.enable"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageNavigate {
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageNavigateResponse {
    pub frame_id: String,
    pub loader_id: Option<String>,
    pub error_text: Option<String>,
}

impl CdpCommand for PageNavigate {
    type Response = PageNavigateResponse;
    fn method_name(&self) -> &'static str {
        "Page.navigate"
    }
}

#[derive(Debug, Serialize)]
pub struct PageGetFrameTree {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageGetFrameTreeResponse {
    pub frame_tree: FrameTree,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameTree {
    pub frame: FrameInfo,
    #[serde(default)]
    pub child_frames: Vec<FrameTree>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameInfo {
    pub id: String,
    pub url: String,
    pub security_origin: Option<String>,
    pub mime_type: Option<String>,
}

impl CdpCommand for PageGetFrameTree {
    type Response = PageGetFrameTreeResponse;
    fn method_name(&self) -> &'static str {
        "Page.getFrameTree"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageCaptureScreenshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PageCaptureScreenshotResponse {
    pub data: String,
}

impl CdpCommand for PageCaptureScreenshot {
    type Response = PageCaptureScreenshotResponse;
    fn method_name(&self) -> &'static str {
        "Page.captureScreenshot"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageReload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_cache: Option<bool>,
}

impl CdpCommand for PageReload {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Page.reload"
    }
}

#[derive(Debug, Serialize)]
pub struct PageBringToFront {}

impl CdpCommand for PageBringToFront {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Page.bringToFront"
    }
}

// ============================================================
// DOM domain
// ============================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomGetDocument {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pierce: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct DomGetDocumentResponse {
    pub root: DomNode,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomNode {
    pub node_id: NodeId,
    pub backend_node_id: BackendNodeId,
    pub node_type: i64,
    pub node_name: String,
    pub node_value: String,
    #[serde(default)]
    pub children: Vec<DomNode>,
    #[serde(default)]
    pub attributes: Vec<String>,
    pub local_name: Option<String>,
}

impl CdpCommand for DomGetDocument {
    type Response = DomGetDocumentResponse;
    fn method_name(&self) -> &'static str {
        "DOM.getDocument"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomQuerySelectorAll {
    pub node_id: NodeId,
    pub selector: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomQuerySelectorAllResponse {
    pub node_ids: Vec<NodeId>,
}

impl CdpCommand for DomQuerySelectorAll {
    type Response = DomQuerySelectorAllResponse;
    fn method_name(&self) -> &'static str {
        "DOM.querySelectorAll"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomDescribeNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<NodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_node_id: Option<BackendNodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pierce: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct DomDescribeNodeResponse {
    pub node: DomNode,
}

impl CdpCommand for DomDescribeNode {
    type Response = DomDescribeNodeResponse;
    fn method_name(&self) -> &'static str {
        "DOM.describeNode"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomResolveNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<NodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_node_id: Option<BackendNodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_group: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DomResolveNodeResponse {
    pub object: RemoteObject,
}

impl CdpCommand for DomResolveNode {
    type Response = DomResolveNodeResponse;
    fn method_name(&self) -> &'static str {
        "DOM.resolveNode"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomGetBoxModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<NodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_node_id: Option<BackendNodeId>,
}

#[derive(Debug, Deserialize)]
pub struct DomGetBoxModelResponse {
    pub model: BoxModel,
}

impl CdpCommand for DomGetBoxModel {
    type Response = DomGetBoxModelResponse;
    fn method_name(&self) -> &'static str {
        "DOM.getBoxModel"
    }
}

// ============================================================
// DOMSnapshot domain
// ============================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomSnapshotCaptureSnapshot {
    pub computed_styles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_dom_rects: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_paint_order: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomSnapshotCaptureSnapshotResponse {
    pub documents: Vec<DocumentSnapshot>,
    pub strings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSnapshot {
    #[serde(rename = "documentURL")]
    pub document_url: i64,
    pub title: i64,
    #[serde(rename = "baseURL")]
    pub base_url: i64,
    pub nodes: NodeTreeSnapshot,
    pub layout: LayoutTreeSnapshot,
    #[serde(default)]
    pub text_boxes: TextBoxSnapshot,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeTreeSnapshot {
    #[serde(default)]
    pub parent_index: Vec<i64>,
    #[serde(default)]
    pub node_type: Vec<i64>,
    #[serde(default)]
    pub shadow_root_type: Option<RareBooleanData>,
    #[serde(default)]
    pub node_name: Vec<i64>,
    #[serde(default)]
    pub node_value: Vec<i64>,
    #[serde(default)]
    pub backend_node_id: Vec<BackendNodeId>,
    #[serde(default)]
    pub attributes: Vec<Vec<i64>>,
    #[serde(default)]
    pub text_value: Option<RareStringData>,
    #[serde(default)]
    pub input_value: Option<RareStringData>,
    #[serde(default)]
    pub input_checked: Option<RareBooleanData>,
    #[serde(default)]
    pub option_selected: Option<RareBooleanData>,
    #[serde(default)]
    pub content_document_index: Option<RareIntegerData>,
    #[serde(default)]
    pub is_clickable: Option<RareBooleanData>,
    #[serde(default)]
    pub current_source_url: Option<RareStringData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTreeSnapshot {
    #[serde(default)]
    pub node_index: Vec<i64>,
    #[serde(default)]
    pub bounds: Vec<Vec<f64>>,
    #[serde(default)]
    pub text: Vec<i64>,
    #[serde(default)]
    pub styles: Vec<Vec<i64>>,
    #[serde(default)]
    pub stacking_contexts: Option<RareBooleanData>,
    #[serde(default)]
    pub paint_orders: Vec<i64>,
    #[serde(default)]
    pub offset_rects: Vec<Vec<f64>>,
    #[serde(default)]
    pub scroll_rects: Vec<Vec<f64>>,
    #[serde(default)]
    pub client_rects: Vec<Vec<f64>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextBoxSnapshot {
    #[serde(default)]
    pub layout_index: Vec<i64>,
    #[serde(default)]
    pub bounds: Vec<Vec<f64>>,
    #[serde(default)]
    pub start: Vec<i64>,
    #[serde(default)]
    pub length: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RareStringData {
    pub index: Vec<i64>,
    pub value: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RareBooleanData {
    pub index: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RareIntegerData {
    pub index: Vec<i64>,
    pub value: Vec<i64>,
}

impl CdpCommand for DomSnapshotCaptureSnapshot {
    type Response = DomSnapshotCaptureSnapshotResponse;
    fn method_name(&self) -> &'static str {
        "DOMSnapshot.captureSnapshot"
    }
}

// ============================================================
// Accessibility domain
// ============================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilityGetFullAXTree {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AccessibilityGetFullAXTreeResponse {
    pub nodes: Vec<AXNode>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AXNode {
    pub node_id: String,
    #[serde(default)]
    pub ignored: bool,
    pub role: Option<AXValue>,
    pub name: Option<AXValue>,
    pub description: Option<AXValue>,
    pub value: Option<AXValue>,
    #[serde(default)]
    pub properties: Vec<AXProperty>,
    #[serde(default)]
    pub child_ids: Vec<String>,
    pub backend_dom_node_id: Option<BackendNodeId>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AXValue {
    #[serde(rename = "type")]
    pub value_type: String,
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AXProperty {
    pub name: String,
    pub value: AXValue,
}

impl CdpCommand for AccessibilityGetFullAXTree {
    type Response = AccessibilityGetFullAXTreeResponse;
    fn method_name(&self) -> &'static str {
        "Accessibility.getFullAXTree"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilityGetPartialAXTree {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<NodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_node_id: Option<BackendNodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_relatives: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AccessibilityGetPartialAXTreeResponse {
    pub nodes: Vec<AXNode>,
}

impl CdpCommand for AccessibilityGetPartialAXTree {
    type Response = AccessibilityGetPartialAXTreeResponse;
    fn method_name(&self) -> &'static str {
        "Accessibility.getPartialAXTree"
    }
}

// ============================================================
// Runtime domain
// ============================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEvaluate {
    pub expression: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_by_value: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub await_promise: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEvaluateResponse {
    pub result: RemoteObject,
    pub exception_details: Option<crate::types::ExceptionDetails>,
}

impl CdpCommand for RuntimeEvaluate {
    type Response = RuntimeEvaluateResponse;
    fn method_name(&self) -> &'static str {
        "Runtime.evaluate"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCallFunctionOn {
    pub function_declaration: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<CallArgument>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_by_value: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub await_promise: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CallArgument {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCallFunctionOnResponse {
    pub result: RemoteObject,
    pub exception_details: Option<crate::types::ExceptionDetails>,
}

impl CdpCommand for RuntimeCallFunctionOn {
    type Response = RuntimeCallFunctionOnResponse;
    fn method_name(&self) -> &'static str {
        "Runtime.callFunctionOn"
    }
}

// ============================================================
// Input domain
// ============================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDispatchMouseEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub x: f64,
    pub y: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_count: Option<i64>,
}

impl CdpCommand for InputDispatchMouseEvent {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Input.dispatchMouseEvent"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDispatchKeyEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unmodified_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_virtual_key_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_virtual_key_code: Option<i64>,
}

impl CdpCommand for InputDispatchKeyEvent {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Input.dispatchKeyEvent"
    }
}

// ============================================================
// Network domain
// ============================================================

#[derive(Debug, Serialize)]
pub struct NetworkGetCookies {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct NetworkGetCookiesResponse {
    pub cookies: Vec<Cookie>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: f64,
    pub size: Option<i64>,
    pub http_only: bool,
    pub secure: bool,
    pub session: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_site: Option<String>,
}

impl CdpCommand for NetworkGetCookies {
    type Response = NetworkGetCookiesResponse;
    fn method_name(&self) -> &'static str {
        "Network.getCookies"
    }
}

#[derive(Debug, Serialize)]
pub struct NetworkSetCookies {
    pub cookies: Vec<CookieParam>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CookieParam {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_site: Option<String>,
}

impl CdpCommand for NetworkSetCookies {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Network.setCookies"
    }
}

#[derive(Debug, Serialize)]
pub struct NetworkDeleteCookies {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl CdpCommand for NetworkDeleteCookies {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Network.deleteCookies"
    }
}

#[derive(Debug, Serialize)]
pub struct NetworkEnable {}

impl CdpCommand for NetworkEnable {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Network.enable"
    }
}

#[derive(Debug, Serialize)]
pub struct NetworkSetExtraHTTPHeaders {
    pub headers: std::collections::HashMap<String, String>,
}

impl CdpCommand for NetworkSetExtraHTTPHeaders {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Network.setExtraHTTPHeaders"
    }
}

// ============================================================
// Emulation domain
// ============================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulationSetDeviceMetricsOverride {
    pub width: i64,
    pub height: i64,
    pub device_scale_factor: f64,
    pub mobile: bool,
}

impl CdpCommand for EmulationSetDeviceMetricsOverride {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Emulation.setDeviceMetricsOverride"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulationSetGeolocationOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<f64>,
}

impl CdpCommand for EmulationSetGeolocationOverride {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Emulation.setGeolocationOverride"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulationSetLocaleOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

impl CdpCommand for EmulationSetLocaleOverride {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Emulation.setLocaleOverride"
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulationSetUserAgentOverride {
    pub user_agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
}

impl CdpCommand for EmulationSetUserAgentOverride {
    type Response = serde_json::Value;
    fn method_name(&self) -> &'static str {
        "Emulation.setUserAgentOverride"
    }
}
