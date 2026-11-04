"""武将词典：696 位武将 -> 534 个「桶」+ 别名（字/号/演员名）。

桶规则（用户裁定）：剥离开头的装饰前缀 ``SP|界|谋|神|魔|族`` 后取本名，
例如 SP关羽 / 界关羽 / 谋关羽 / 神关羽 -> 关羽（一桶多 gid）；找不到素材的桶留空。
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field

from .paths import ALIASES_JSON, HEROES_JSON

PREFIXES = ("SP", "sp", "Sp", "sP", "界", "谋", "神", "魔", "族")
CJK_RE = re.compile(r"[\u4e00-\u9fff]+")
ZI_RE = re.compile(r"[，,。；;]字([\u4e00-\u9fff]{1,2})[，,。；;、]")


def strip_prefix(name: str) -> str:
    out = name.strip()
    changed = True
    while changed:
        changed = False
        for p in PREFIXES:
            if out.startswith(p) and len(out) > len(p) + 1:
                out = out[len(p) :]
                changed = True
    return out


def normalize(text: str) -> str:
    """只留汉字，去掉空格/标点/数字/括号等噪声。"""
    return "".join(CJK_RE.findall(text or ""))


def levenshtein(a: str, b: str) -> int:
    if a == b:
        return 0
    if not a or not b:
        return max(len(a), len(b))
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]


@dataclass
class Match:
    bucket: str
    kind: str  # exact | alias | contains | fuzzy
    score: float
    token: str


@dataclass
class Dictionary:
    buckets: dict[str, list[int]] = field(default_factory=dict)  # 桶名 -> gid 列表
    tokens: dict[str, str] = field(default_factory=dict)  # 归一化 token -> 桶名
    gid_name: dict[int, str] = field(default_factory=dict)

    @property
    def bucket_count(self) -> int:
        return len(self.buckets)

    def match(self, text: str) -> Match | None:
        t = normalize(text)
        if not t:
            return None
        # 1) 精确
        if t in self.tokens:
            b = self.tokens[t]
            return Match(b, "alias" if b != t else "exact", 1.0, t)
        # 2) 包含（OCR 常把前后文字一起框进去，限制多余字数 <= 2）
        best: Match | None = None
        for tok, bucket in self.tokens.items():
            if len(tok) >= 2 and tok in t and len(t) - len(tok) <= 2:
                cand = Match(bucket, "contains", 0.85, tok)
                if best is None or len(cand.token) > len(best.token):
                    best = cand
        if best:
            return best
        # 宁漏不误：不做编辑距离容错（OCR 错字宁可漏，不产生误挂）
        return None


    def find_all(self, text: str) -> list[tuple[str, str]]:
        """在一整句台词里找出所有武将名（长 token 优先，同一桶只留一个）。"""
        t = normalize(text)
        if len(t) < 2:
            return []
        cand: list[tuple[str, str]] = []
        for tok, bucket in self.tokens.items():
            if len(tok) >= 2 and tok in t:
                cand.append((bucket, tok))
        cand.sort(key=lambda x: -len(x[1]))
        out: list[tuple[str, str]] = []
        for bucket, tok in cand:
            if any(b == bucket for b, _ in out):
                continue
            if any(len(t2) > len(tok) and tok in t2 for _, t2 in out):
                continue
            out.append((bucket, tok))
        return out


def load_dictionary(include_actors: bool = True) -> Dictionary:
    data = json.loads(HEROES_JSON.read_text(encoding="utf-8"))
    heroes = data["heroes"]

    buckets: dict[str, list[int]] = {}
    gid_name: dict[int, str] = {}
    tokens: dict[str, str] = {}

    for h in heroes:
        gid = int(h["gid"])
        name = h["name"]
        gid_name[gid] = name
        base = strip_prefix(name)
        buckets.setdefault(base, []).append(gid)
        tokens.setdefault(normalize(base), base)

    # 自动别名：biography 里的「字X」
    auto_alias = 0
    for h in heroes:
        base = strip_prefix(h["name"])
        bio = h.get("biography") or ""
        for zi in ZI_RE.findall(bio):
            zi = normalize(zi)
            if len(zi) >= 2 and zi not in tokens:
                tokens[zi] = base
                auto_alias += 1

    # 人工别名
    manual = json.loads(ALIASES_JSON.read_text(encoding="utf-8"))
    manual_alias = 0
    for alias, target in manual.items():
        if alias.startswith("_"):
            continue
        target = strip_prefix(target)
        if alias in manual and not include_actors and target in _ACTOR_TOKENS:
            continue
        norm = normalize(alias)
        if norm and norm not in tokens:
            tokens[norm] = target
            manual_alias += 1

    return Dictionary(buckets=buckets, tokens=tokens, gid_name=gid_name)


# 演员名 token（用于正文匹配时可选排除）
_ACTOR_TOKENS = set()


def is_actor_alias(token: str) -> bool:
    return token in _ACTOR_TOKENS


def summary(d: Dictionary) -> str:
    dup = sum(1 for v in d.buckets.values() if len(v) > 1)
    return (
        f"桶 {len(d.buckets)} 个（多版本合并 {dup} 个）、词典 token {len(d.tokens)} 条、"
        f"武将 {sum(len(v) for v in d.buckets.values())} 位"
    )
