"""视觉处理：ROI 切分、文字行检测（免 OCR 预筛）、清晰度 / dHash、字幕 OCR 封装。

设计要点：
- 先用「亮像素 + 水平边缘」的形态学连通域找出**文字行**，没找到就不调用 OCR（省 90% 以上算力）
- 同一字幕停留多帧时用 mask 指纹复用上次 OCR 结果（2fps 采样下再省一半）
- OCR 模型权重放 C 盘（``SGS_RAPIDOCR_DIR``），venv 与产物在 D 盘
"""

from __future__ import annotations

import numpy as np
import cv2

from .paths import BOTTOM_BAND, RAPIDOCR_DIR, ROI_LEFT, ROI_RIGHT

SIDES = ("L", "R")

# 侧栏纵向取景带（排除底部台词字幕与顶部台标；老版三国台词字幕在最下面 ~14%）
SIDE_BAND = (0.16, 0.72)

# 固定叠加物遮罩（整帧比例：CCTV8 台标在左上角）
OVERLAY_RECTS = ((0.00, 0.00, 0.24, 0.13),)


def side_spans(width: int, height: int | None = None) -> dict[str, tuple[int, int, int, int]]:
    """返回左右侧栏的 (x0, y0, x1, y1)。"""
    y0 = 0 if height is None else int(height * SIDE_BAND[0])
    y1 = height if height is None else int(height * SIDE_BAND[1])
    return {
        "L": (int(width * ROI_LEFT[0]), y0, int(width * ROI_LEFT[1]), y1),
        "R": (int(width * ROI_RIGHT[0]), y0, int(width * ROI_RIGHT[1]), y1),
    }


def bottom_span(width: int, height: int) -> tuple[int, int, int, int]:
    """底部台词字幕带。"""
    return (0, int(height * BOTTOM_BAND[0]), width, int(height * BOTTOM_BAND[1]))


def detect_subtitle_line(gray: np.ndarray, min_h: int = 10, max_h: int = 90) -> tuple[int, int, int, int] | None:
    """底部台词字幕专用：按行投影找白字行（字幕常横跨整宽，不能用宽高比筛选）。"""
    h, w = gray.shape[:2]
    bright = gray > 168
    rows = bright.sum(axis=1)
    thr = max(6, int(w * 0.022))
    hit = np.where(rows > thr)[0]
    if hit.size == 0:
        return None
    # 分组为连续行带，取行密度最高的那一段
    groups: list[tuple[int, int]] = []
    start = prev = int(hit[0])
    for r in hit[1:]:
        r = int(r)
        if r - prev <= 2:
            prev = r
            continue
        groups.append((start, prev))
        start = prev = r
    groups.append((start, prev))
    groups = [(a, b) for a, b in groups if min_h <= (b - a + 1) <= max_h]
    if not groups:
        return None
    y0, y1 = max(groups, key=lambda g: int(rows[g[0] : g[1] + 1].sum()))
    y0, y1 = max(0, y0 - 2), min(h, y1 + 3)
    cols = bright[y0:y1].any(axis=0)
    xs = np.where(cols)[0]
    if xs.size == 0:
        return None
    x0, x1 = int(xs[0]), int(xs[-1]) + 1
    if x1 - x0 < 12:
        return None
    return (x0, y0, x1 - x0, y1 - y0)


def detect_text_lines_vertical(
    gray: np.ndarray, min_h: int = 16, max_h: int | None = None
) -> tuple[np.ndarray, list[tuple[tuple[int, int, int, int], tuple[int, int, int, int]]]]:
    """竖排文字检测：把图转 90° 当横排跑，返回 (转置图, [(转置坐标 box, 原图坐标 box)])。

    老版三国的人物介绍字幕是**竖排**（左边一列姓名、右边一列「字 X」），横排检测器完全检不到。
    """
    h, w = gray.shape[:2]
    g2 = cv2.rotate(gray, cv2.ROTATE_90_COUNTERCLOCKWISE)
    boxes = detect_text_lines(g2, min_h=min_h, max_h=max_h, max_ratio=6.0, min_ratio=0.9, max_fill=0.90)
    out: list[tuple[tuple[int, int, int, int], tuple[int, int, int, int]]] = []
    for rx, ry, rw, rh in boxes:
        # ROTATE_90_COUNTERCLOCKWISE：src(x,y) -> dst(y, w-1-x)
        sx = w - ry - rh
        sy = rx
        out.append(((rx, ry, rw, rh), (sx, sy, rh, rw)))
    return g2, out


def roi_text_gate(gray: np.ndarray, min_bright: int = 55) -> bool:
    """侧栏“可能有字”的廉价开关：亮像素足够多 + 水平边缘量够。失败就丢，零 OCR 开销。"""
    bright = gray > 178
    n = int(bright.sum())
    if n < min_bright:
        return False
    edges = cv2.convertScaleAbs(cv2.Sobel(gray, cv2.CV_16S, 1, 0, ksize=3))
    return float((edges > 30).mean()) > 0.02 and n < 0.25 * gray.size


def mask_overlay_in_roi(
    gray: np.ndarray,
    frame_w: int,
    frame_h: int,
    roi: tuple[int, int, int, int],
    rects=OVERLAY_RECTS,
) -> np.ndarray:
    """把整帧坐标系的台标矩形映射到 ROI 局部坐标并涂黑（原地修改并返回）。"""
    rx0, ry0, rx1, ry1 = roi
    rh, rw = gray.shape[:2]
    for ox0, oy0, ox1, oy1 in rects:
        gx0 = max(0, int(frame_w * ox0) - rx0)
        gx1 = min(rw, int(frame_w * ox1) - rx0)
        gy0 = max(0, int(frame_h * oy0) - ry0)
        gy1 = min(rh, int(frame_h * oy1) - ry0)
        if gx1 > gx0 and gy1 > gy0:
            gray[gy0:gy1, gx0:gx1] = 0
    return gray


def detect_text_lines(
    gray: np.ndarray,
    min_h: int = 16,
    max_h: int | None = None,
    max_ratio: float = 6.0,
    min_ratio: float = 1.4,
    max_fill: float = 0.70,
) -> list[tuple[int, int, int, int]]:
    """返回文字行 bbox 列表（x, y, w, h），按面积倒序。"""
    h, w = gray.shape[:2]
    if max_h is None:
        max_h = max(28, min(56, int(h * 0.14)))
    # 亮色文字（字幕为白字 + 描边）
    bright = (gray > 185).astype(np.uint8)
    if bright.sum() < 40:
        return []
    gx = cv2.Sobel(gray, cv2.CV_16S, 1, 0, ksize=3)
    edges = cv2.convertScaleAbs(gx)
    mask = ((bright > 0) & (edges > 35)).astype(np.uint8) * 255
    # 水平膨胀把单字连成一行
    mask = cv2.dilate(mask, cv2.getStructuringElement(cv2.MORPH_RECT, (11, 3)), iterations=1)
    mask = cv2.morphologyEx(mask, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_RECT, (15, 3)))
    n, _, stats, _ = cv2.connectedComponentsWithStats(mask, connectivity=8)
    out: list[tuple[int, int, int, int]] = []
    for i in range(1, n):
        x, y, bw, bh, area = stats[i]
        if bh < min_h or bh > max_h:
            continue
        if bw < int(min_ratio * bh):  # 横排=宽应大于高×1.4；竖排（转置后）两列可能接近方形
            continue
        if bw > int(bh * max_ratio):  # 横跨整个侧栏的长句（台词/歌词）不要
            continue
        fill = area / float(bw * bh)
        if fill < 0.12 or fill > max_fill:  # 太稀疏/太实心都不是文字行
            continue
        out.append((int(x), int(y), int(bw), int(bh)))
    out.sort(key=lambda b: b[2] * b[3], reverse=True)
    return out[:3]


def union_box(boxes: list[tuple[int, int, int, int]], pad: int, w: int, h: int) -> tuple[int, int, int, int]:
    x0 = min(b[0] for b in boxes)
    y0 = min(b[1] for b in boxes)
    x1 = max(b[0] + b[2] for b in boxes)
    y1 = max(b[1] + b[3] for b in boxes)
    return (max(0, x0 - pad), max(0, y0 - pad), min(w, x1 + pad), min(h, y1 + pad))


def sharpness(gray: np.ndarray) -> float:
    return float(cv2.Laplacian(gray, cv2.CV_64F).var())


def dhash(gray: np.ndarray, size: int = 8) -> int:
    small = cv2.resize(gray, (size + 1, size), interpolation=cv2.INTER_AREA).astype(np.int16)
    bits = (small[:, 1:] > small[:, :-1]).flatten()
    v = 0
    for b in bits:
        v = (v << 1) | int(b)
    return v


def hamming(a: int, b: int) -> int:
    return bin(a ^ b).count("1")


def mask_signature(gray: np.ndarray, size: int = 8) -> int:
    """用二值化后的形状做指纹：字幕内容不变时指纹稳定（用于复用 OCR）。"""
    _, m = cv2.threshold(gray, 185, 255, cv2.THRESH_BINARY)
    return dhash(m, size)


class OcrEngine:
    """RapidOCR（PP-OCRv3 mobile, onnxruntime, 纯 CPU）。"""

    def __init__(self, models_dir=RAPIDOCR_DIR) -> None:
        from rapidocr_onnxruntime import RapidOCR  # 延迟导入，避免 probe 也拖 2s

        det = models_dir / "ch_PP-OCRv3_det_infer.onnx"
        rec = models_dir / "ch_PP-OCRv3_rec_infer.onnx"
        cls = models_dir / "ch_ppocr_mobile_v2.0_cls_infer.onnx"
        kwargs: dict = {"use_angle_cls": True, "show_log": False, "det_limit_side_len": 480}
        if det.exists():
            kwargs["det_model_path"] = str(det)
        if rec.exists():
            kwargs["rec_model_path"] = str(rec)
        if cls.exists():
            kwargs["cls_model_path"] = str(cls)
        self._engine = RapidOCR(**kwargs)
        self.calls = 0

    def recognize(self, bgr: np.ndarray, line_mode: bool = True) -> tuple[str, float]:
        """识别。``line_mode=True`` 时只走 rec（已由形态学/投影定位到单行，快 3–5 倍）。"""
        self.calls += 1
        text, conf = self._run(bgr, use_det=not line_mode)
        if line_mode and not text:
            # 只在完全无字时补一次带 det 的识别（避免逐帧双倍开销）
            self.calls += 1
            t2, c2 = self._run(bgr, use_det=True)
            if t2:
                return t2, c2
        return text, conf

    def _run(self, bgr: np.ndarray, use_det: bool) -> tuple[str, float]:
        items = self.recognize_boxes(bgr, use_det=use_det)
        if not items:
            return "", 0.0
        return "".join(i["text"] for i in items), sum(i["score"] for i in items) / len(items)

    def recognize_boxes(self, bgr: np.ndarray, use_det: bool = True) -> list[dict]:
        """返回 [{text, score, box:(x,y,w,h)}]；竖排文字交给 det 自己处理旋转（不能整体旋转图）。"""
        self.calls += 1
        try:
            result, _ = self._engine(bgr, use_det=use_det, use_cls=False, use_rec=True)
        except TypeError:  # 兼容不同版本的签名
            result, _ = self._engine(bgr)
        if not result:
            return []
        out: list[dict] = []
        for item in result:
            try:
                box, text, score = item[0], item[1], float(item[2])
            except Exception:  # noqa: BLE001
                continue
            if not text:
                continue
            xs = [p[0] for p in box]
            ys = [p[1] for p in box]
            out.append(
                {
                    "text": str(text),
                    "score": score,
                    "box": (int(min(xs)), int(min(ys)), int(max(xs) - min(xs)), int(max(ys) - min(ys))),
                }
            )
        return out
