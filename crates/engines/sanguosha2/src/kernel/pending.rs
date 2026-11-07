//! Pending + **类型级强制出口**。
//!
//! 不变量（plan §2.4）：任何 pending 都必须有「放弃/取消」出口。
//! 实现方式：`Pending` 字段私有、只由 `PendingBuilder` 构造；`build()` 只在
//! `PendingBuilder<HasEscape>` 上存在 → **没有出口就编译不过**（不是靠人记）。

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
    pub fn has_option(&self, id: &str) -> bool {
        self.options.iter().any(|o| o.id == id)
    }
}

/// 类型状态：尚未声明出口。
pub struct NoEscape;
/// 类型状态：已声明出口。
pub struct HasEscape;

pub struct PendingBuilder<S> {
    actor: usize,
    kind: String,
    skill: Option<String>,
    options: Vec<PendingOption>,
    data: Value,
    escape_id: Option<String>,
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
    /// 声明出口（必填步骤）。`id` 建议固定为 `pass` 或 `cancel`。
    pub fn escape(mut self, label: impl Into<String>) -> PendingBuilder<HasEscape> {
        let id = "pass".to_string();
        self.options.push(PendingOption {
            id: id.clone(),
            label: label.into(),
        });
        self.escape_id = Some(id);
        PendingBuilder {
            actor: self.actor,
            kind: self.kind,
            skill: self.skill,
            options: self.options,
            data: self.data,
            escape_id: self.escape_id,
            _state: PhantomData,
        }
    }

    /// 「取消」语义的出口（用于多选确认类交互）。
    pub fn escape_with(
        mut self,
        id: &'static str,
        label: impl Into<String>,
    ) -> PendingBuilder<HasEscape> {
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
        }
    }
}
