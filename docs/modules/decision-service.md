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
- `noul`（是/否）**固定 true/false 两项**（渲染为 yes/no），返回 `{ type:"noul", noul: P(true) }`，**无 confidence/probabilities**，需调用方阈值化；`bool` 是 `noul` 的别名。现用于聊天链接抓取判断（`decideLinkFetch`，见 `docs/modules/agent-gateway.md`）与**人物画像把关**（`profile/gate.ts`：候选事实是否写入长期画像，ADR-0007）

## 置信度

- `choice` 置信度 = `(p_max − 1/n) / (1 − 1/n)`（TypeSafe 定义）
- 阈值由 agent-gateway 配置化，低于阈值走兜底

## 中文处理策略（已实测：中文直传可用，暂不引入翻译）

- StartLux-Decision 虽为英文微调，但中文玩家消息的**零样本跳语言**表现经实测与英文持平（详见下方结果）。
- **实测结论（2026-10，Q8_0 GGUF + 本机 RTX 5070 Ti）**：
  - 意图分类（4 类，criteria 英文）：中文 6/7 正确，高置信项均在 0.90+；唯一错项（“这局规则是什么？”）中英文置信度均 < 0.6，会触发兜底，符合设计。
  - 动作选择（中文局面 + 中文合法动作描述）：4/4 正确，置信度 0.96–0.97；“查看排行榜”类写操作 intent 置信度偏低（0.50），正好走兜底。
  - `noul`（是/否）：方向正确且置信度分化明显（动作 0.82 / 非动作 0.01 / 开新局请求 0.99）。
  - `score`（分级）：方向正确（明确消息 2.61 vs 含糊 0.73），但置信度偏低（0.34/0.46），阈值需按场景调低。
  - criteria 也用中文（最坏情况）：3/3 正确，置信度 0.90–0.99。
- **结论：当前阶段不引入本地 MT 翻译**，中文直接传入 `state` 即可；若后续出现特定难样本再评估。
- （原备选方案，暂不实施）若未来需要：本地 MT 小模型翻译（NLLB / Qwen-MT，zh→en，~0.1–0.4s）或低置信才翻译的两级路由。翻译一律用**本地小模型**，不用远程 LLM（远程会拖慢游戏决策路径、增加隐私出站）。

## 部署要点

- Windows：`llama-server`（llama.cpp，GGUF）+ Python `gguf_server.py`
  - `gguf_server.py` 需 torch + transformers（仅 tokenizer 与 logprob 归一化），**无需** flash-linear-attention / causal-conv1d（二者 Linux-only，仅直连 torch 路径需要）
- 示例：

```bash
llama-server -m StartLux-Decision-4B-Q8_0.gguf -ngl 99 -c 16384 --parallel 4 --port 8081
python -m startlux_decision.gguf_server --model-dir StartLux-Decision-4B-GGUF \
       --llama http://127.0.0.1:8081 --port 8090
```

- **一键启动（推荐）**：`apps/scripts/start-decision-service.mjs` 按顺序拉起上两层服务、探活并转发日志（前台运行，Ctrl+C 一并停止）

```bash
# 前台运行（LLAMA_SERVER_BIN 未配时默认取 PATH 中的 llama-server）
LLAMA_SERVER_BIN=/c/llm/llama.cpp-master/build/bin/Release/llama-server.exe \
  node apps/scripts/start-decision-service.mjs

# 后台常驻
LLAMA_SERVER_BIN=/c/llm/llama.cpp-master/build/bin/Release/llama-server.exe \
  nohup node apps/scripts/start-decision-service.mjs > /tmp/decision-svc.log 2>&1 &
```

  可覆盖的环境变量：`MODEL_DIR` / `LLAMA_SERVER_BIN` / `LLAMA_PORT` / `DECISION_PORT` / `NGL` / `CTX_SIZE` / `PARALLEL` / `PYTHON_BIN`（详见脚本头部注释）。
- **健康探针**：`node apps/scripts/check-decision-service.mjs`（检查 llama-server + 决策服务 + 样例推理；健康 exit 0，异常 exit 1；`--json` 机读，`PROBE_SAMPLE=0` 跳过推理）
- 模型权重（`StartLux-Decision-v1-4B-Q8_0-GGUF/`，约 4.5GB）**不入库**，已在根 `.gitignore` 忽略
- 4B Q8_0 ≈ 4.3GB 显存；延迟远低于被动回复窗口，非瓶颈
- GGUF 只读文本（视觉塔不可用）；本项目纯文本场景，够用

## License 注意

- 权重 **CC BY-NC 4.0（非商用）**；商用需向 StartLux Labs 单独授权（见 ADR-0002）
