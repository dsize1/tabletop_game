"""sgs_media：1994《三国演义》人物介绍字幕素材采集（CCTV 源 + RapidOCR，纯 CPU）。

分工：
- 模型权重 → C 盘（``SGS_RAPIDOCR_DIR``，默认 ``C:/llm/models/rapidocr``）
- 产物/中间产物 → D 盘仓库内 ``data/media/sgs``（被 .gitignore 忽略）
- 运行前置：``node apps/scripts/media-gate.mjs --acquire``（项目服务全停 + 独占锁）
"""

__all__ = ["cctv", "dicts", "pipeline", "report", "vision", "paths"]
