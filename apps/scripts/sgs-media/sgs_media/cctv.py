"""CCTV（tv.cctv.com / vdn.apps.cntv.cn）取流。

实测链路（2026-11）：
    页面 → ``guid = "<32hex>"``
        → ``https://vdn.apps.cntv.cn/api/getHttpVideoInfo.do?pid=<guid>``（JSON：title/totalLength/hls_url）
        → HLS master ``/asp/hls/main/0303000a/3/default/<guid>/main.m3u8?maxbr=2048``
        → 变体 ``/asp/hls/{450|1200|2000}/0303000a/3/default/<guid>/{br}.m3u8``（VOD，10–15s 一段 .ts）

全程无需登录、无需 ffmpeg（PyAV 直接解 .ts）。
"""

from __future__ import annotations

import json
import re
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urljoin

UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36"
GUID_RE = re.compile(r'guid\s*=\s*"([0-9a-fA-F]{32})"')
HLS_TPL = "https://newcntv.qcloudcdn.com/asp/hls/{br}/0303000a/3/default/{guid}/{br}.m3u8"


class CctvError(RuntimeError):
    pass


def _get(url: str, timeout: float = 30.0) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Referer": "https://tv.cctv.com/"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return resp.read()


def fetch_guid(page_url: str) -> str:
    html = _get(page_url).decode("utf-8", errors="ignore")
    m = GUID_RE.search(html)
    if not m:
        raise CctvError(f"页面里找不到 guid：{page_url}")
    return m.group(1).lower()


def fetch_info(guid: str) -> dict:
    raw = _get(f"https://vdn.apps.cntv.cn/api/getHttpVideoInfo.do?pid={guid}")
    info = json.loads(raw.decode("utf-8", errors="ignore"))
    if info.get("ack") != "yes":
        raise CctvError(f"getHttpVideoInfo 返回异常：{info.get('ack')} / {info.get('msg')}")
    return info


def hls_variants(guid: str) -> list[str]:
    return [HLS_TPL.format(br=br, guid=guid) for br in (2000, 1200, 450)]


def pick_variant(guid: str, prefer: int = 2000) -> str:
    """按 prefer 顺序探测可用变体，返回第一个 200 的 m3u8 URL。"""
    order = [prefer] + [br for br in (2000, 1200, 450) if br != prefer]
    errors = []
    for br in order:
        url = HLS_TPL.format(br=br, guid=guid)
        try:
            text = _get(url, timeout=20).decode("utf-8", errors="ignore")
        except Exception as e:  # noqa: BLE001 - 探测式回退
            errors.append(f"{br}: {e}")
            continue
        if "#EXTM3U" in text:
            return url
    raise CctvError("所有码率变体都不可用：" + "; ".join(errors))


@dataclass
class Segment:
    index: int
    duration: float
    url: str


def parse_playlist(m3u8_url: str) -> list[Segment]:
    text = _get(m3u8_url).decode("utf-8", errors="ignore")
    if "#EXT-X-STREAM-INF" in text:
        # master playlist：取最后一行非注释 URI 作为子播放列表
        uri = next((ln.strip() for ln in reversed(text.splitlines()) if ln.strip() and not ln.startswith("#")), None)
        if not uri:
            raise CctvError("master playlist 解析失败")
        return parse_playlist(urljoin(m3u8_url, uri))
    segs: list[Segment] = []
    dur = 0.0
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        if line.startswith("#EXTINF:"):
            dur = float(line[len("#EXTINF:") :].split(",")[0])
        elif not line.startswith("#"):
            segs.append(Segment(index=len(segs), duration=dur, url=urljoin(m3u8_url, line)))
            dur = 0.0
    if not segs:
        raise CctvError("playlist 里没有分片")
    return segs


def download_segment(url: str, dest: Path, retries: int = 3) -> Path:
    if dest.exists() and dest.stat().st_size > 0:
        return dest
    dest.parent.mkdir(parents=True, exist_ok=True)
    last: Exception | None = None
    for _ in range(retries):
        try:
            data = _get(url, timeout=60)
            if not data:
                raise CctvError("空分片")
            tmp = dest.with_suffix(dest.suffix + ".part")
            tmp.write_bytes(data)
            tmp.replace(dest)
            return dest
        except Exception as e:  # noqa: BLE001 - 网络重试
            last = e
    raise CctvError(f"分片下载失败：{url}（{last}）")


def resolve(page_url: str, prefer: int = 2000) -> dict:
    """页面 URL -> {guid, info, m3u8, segments, title, total}"""
    guid = fetch_guid(page_url)
    info = fetch_info(guid)
    m3u8 = pick_variant(guid, prefer=prefer)
    segs = parse_playlist(m3u8)
    total = float(info.get("video", {}).get("totalLength") or sum(s.duration for s in segs))
    return {
        "guid": guid,
        "info": info,
        "title": info.get("title", ""),
        "total": total,
        "m3u8": m3u8,
        "segments": segs,
    }
