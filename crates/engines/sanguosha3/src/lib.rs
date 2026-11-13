//! 三国杀内核 V3 —— 骨架类型系统（M1 第一步）。
//!
//! 目标（ADR-0042 / `memory_palace/plan/next-v3-rebuild.md` §3-M1）：
//! 把 v2 反复踩坑后「事后补丁式」的机制，从第一天起固化为**类型 + 不变量 + 门禁**。
//!
//! 本 crate 是纯内核，不注册 `game-engine-mcp`、不实现 `EnginePlugin`（那是 M2/M3 的事）。
//! 第一步只交付三样，且各有红绿测试：
//! 1. **牌区唯一归属 + 守恒**（`kernel::zone`）：每张牌任意时刻恰好属于一个 `Zone`；
//! 2. **事件 + 窗口 + 主体**（`kernel::event`）：每个订阅声明 `Window` 与 `Subject`；
//! 3. **交互描述符 + 类型级出口**（`kernel::pending`）：没有出口的 pending 编译不过。
//!
//! 与 v2 的差异见 `Cargo.toml` 顶部注释。

pub mod kernel;
