# decision-service（SystemOne 决策服务）

- **模型**：StartLux-Decision-4B（Q8_0 GGUF）
- **形态**：本地 HTTP 服务，暴露 `/v1/systemone`（TypeSafe 格式）
- **运行**：`llama-server`（GGUF 前向）+ `startlux_decision.gguf_server`（typed-decision 逻辑）

## 职责

- 对给定 `state`（文本/JSON）回答 typed 问题：`choice`（多选一）/ `noul`（是/否）/ `score`（分级）
- 每个选项返回**概率** + **置信度**（TypeSafe 公式）
- **只建议，不写状态**：无持久化、无副作用，纯无状态函数

## 接口契约

```
POST /v1/systemone
{
  "state":    { ... },            // 局面 + 合法动作 + 玩家消息（JSON）
  "questions": {
    "intent": { "type": "choice", "criteria": { "chat": "...", "dev": "...", "game_action": "..." } },
    "action": { "type": "choice", "criteria": { "<action_id>": "<desc>", ... } }
  }
}
→ {
  "answers": {
    "intent": { "answer": "...", "confidence": 0.87, "probabilities": {...} },
    "action": { "answer": "...", "confidence": 0.62, "probabilities": {...} }
  },
  "usage": {...}
}
```

- **单次前向支持多问题**：意图 + 动作可合一为一次调用
- **单题选项上限 26**（A~Z 字母 token 读出）；超限自动走 `wide_choice` 两阶段（25 个一组、留 3）

## 置信度

- `choice` 置信度 = `(p_max − 1/n) / (1 − 1/n)`（TypeSafe 定义）
- 阈值由 agent-gateway 配置化，低于阈值走兜底

## 中文处理策略（待验证，暂不实现）

- StartLux-Decision 为英文微调，中文玩家消息是**零样本跨语言**，效果需在验证阶段实测。
- 若验证阶段中文不理想，按以下顺序启用（**本次不实现，确认需要后再实施**）：
  1. **本地 MT 小模型翻译**（NLLB / Qwen-MT 类，zh→en，~0.1–0.4s）：中文消息译成英文，**中英同时传入 `state`**；避免远程 LLM 翻译的 1–3s 延迟与额外隐私出站。
  2. **低置信才翻译（两级路由）**：中文直传 → 置信度达标即返回（快路径）；低于阈值才翻译并重决策（慢路径，只在难样本）。
- 翻译一律用**本地小模型**，不用远程 LLM（远程会拖慢游戏路径、威胁被动回复窗口）。

## 部署要点

- Windows：`llama-server`（llama.cpp，GGUF）+ Python `gguf_server.py`
  - `gguf_server.py` 需 torch + transformers（仅 tokenizer 与 logprob 归一化），**无需** flash-linear-attention / causal-conv1d（二者 Linux-only，仅直连 torch 路径需要）
- 示例：

```bash
llama-server -m StartLux-Decision-4B-Q8_0.gguf -ngl 99 -c 16384 --parallel 4 --port 8081
python -m startlux_decision.gguf_server --model-dir StartLux-Decision-4B-GGUF \
       --llama http://127.0.0.1:8081 --port 8090
```

- 4B Q8_0 ≈ 4.3GB 显存；延迟远低于被动回复窗口，非瓶颈
- GGUF 只读文本（视觉塔不可用）；本项目纯文本场景，够用

## License 注意

- 权重 **CC BY-NC 4.0（非商用）**；商用需向 StartLux Labs 单独授权（见 ADR-0002）
