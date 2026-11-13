//! 交互描述符（pending）+ **类型级强制出口**。
//!
//! 不变量（constitution I1 询问完整性）：
//! 1. 任何 pending 都必须有「放弃/取消」出口；`build()` 只在 `PendingBuilder<HasEscape>` 上存在
//!    → **没有出口就编译不过**（不是靠人记）。
//! 2. 强制选择类步骤（展示/选牌/弃至上限）用 `escape_mandatory`：出口只是类型占位，
//!    端侧不展示，**提交出口被拒绝**（引擎报非法，不得代玩家选第一个实质选项）。
//! 3. 只有「合法集为空」才允许不产生 pending；有手牌必须显式询问（`must_ask`）。

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    actor: usize,
    kind: String,
    skill: Option<String>,
    options: Vec<PendingOption>,
    data: Value,
    /// 出口选项 id（`pass` / `cancel`），由 builder 保证存在。
    escape_id: String,
    /// 强制选择类：出口只作类型占位，端侧不展示，提交出口被拒绝。
    #[serde(default)]
    mandatory: bool,
}

impl Pending {
    pub fn actor(&self) -> usize {
        self.actor
    }
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn skill(&self) -> Option<&str> {
        self.skill.as_deref()
    }
    pub fn options(&self) -> &[PendingOption] {
        &self.options
    }
    pub fn data(&self) -> &Value {
        &self.data
    }
    pub fn escape_id(&self) -> &str {
        &self.escape_id
    }
    /// 出口是否为「占位」（强制选择类）。
    pub fn escape_hidden(&self) -> bool {
        self.mandatory
    }
    pub fn has_option(&self, id: &str) -> bool {
        self.options.iter().any(|o| o.id == id)
    }
}

/// 询问完整性单一判定入口（I1）：只有「合法集为空」才不问。
///
/// - `has_option` = false（合法集为空）→ 不询问，直接结算；
/// - `has_option` = true（有手牌/有技能/有装备替代）→ 必须产生 pending，即使唯一选项是 `pass`。
pub fn must_ask(has_option: bool) -> bool {
    has_option
}

pub struct NoEscape;
pub struct HasEscape;

pub struct PendingBuilder<S> {
    actor: usize,
    kind: String,
    skill: Option<String>,
    options: Vec<PendingOption>,
    data: Value,
    escape_id: Option<String>,
    mandatory: bool,
    _state: PhantomData<S>,
}

impl PendingBuilder<NoEscape> {
    pub fn new(actor: usize, kind: &'static str) -> Self {
        Self {
            actor,
            kind: kind.to_string(),
            skill: None,
            options: Vec::new(),
            data: Value::Null,
            escape_id: None,
            mandatory: false,
            _state: PhantomData,
        }
    }
}

impl<S> PendingBuilder<S> {
    pub fn skill(mut self, skill: &'static str) -> Self {
        self.skill = Some(skill.to_string());
        self
    }

    pub fn option(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.options.push(PendingOption {
            id: id.into(),
            label: label.into(),
        });
        self
    }

    pub fn data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }
}

impl PendingBuilder<NoEscape> {
    /// 可选步骤的放弃出口（濒死救援 / 无懈 / 响应闪 / 火攻弃牌…）。
    pub fn escape(mut self, label: impl Into<String>) -> PendingBuilder<HasEscape> {
        self.options.push(PendingOption {
            id: "pass".to_string(),
            label: label.into(),
        });
        self.escape_id = Some("pass".to_string());
        PendingBuilder {
            actor: self.actor,
            kind: self.kind,
            skill: self.skill,
            options: self.options,
            data: self.data,
            escape_id: self.escape_id,
            mandatory: false,
            _state: PhantomData,
        }
    }

    /// 强制选择类（展示手牌 / 选牌 / 弃至上限 / 取五谷牌）：出口只满足类型状态。
    /// 提交该出口必须被拒绝（引擎报非法），不得代选。
    pub fn escape_mandatory(mut self, label: impl Into<String>) -> PendingBuilder<HasEscape> {
        self.options.push(PendingOption {
            id: "pass".to_string(),
            label: label.into(),
        });
        self.escape_id = Some("pass".to_string());
        PendingBuilder {
            actor: self.actor,
            kind: self.kind,
            skill: self.skill,
            options: self.options,
            data: self.data,
            escape_id: self.escape_id,
            mandatory: true,
            _state: PhantomData,
        }
    }

    /// 「取消」语义的出口（多选确认类）。
    pub fn escape_with(mut self, id: &'static str, label: impl Into<String>) -> PendingBuilder<HasEscape> {
        self.options.push(PendingOption {
            id: id.to_string(),
            label: label.into(),
        });
        self.escape_id = Some(id.to_string());
        PendingBuilder {
            actor: self.actor,
            kind: self.kind,
            skill: self.skill,
            options: self.options,
            data: self.data,
            escape_id: self.escape_id,
            mandatory: false,
            _state: PhantomData,
        }
    }
}

impl PendingBuilder<HasEscape> {
    pub fn build(self) -> Pending {
        debug_assert!(
            self.options
                .iter()
                .any(|o| Some(&o.id) == self.escape_id.as_ref()),
            "pending 必须包含出口选项"
        );
        Pending {
            actor: self.actor,
            kind: self.kind,
            skill: self.skill,
            options: self.options,
            data: self.data,
            escape_id: self.escape_id.expect("HasEscape 状态必有出口"),
            mandatory: self.mandatory,
        }
    }
}
