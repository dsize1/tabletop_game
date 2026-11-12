# 授予技 / 无对齐单条目的技能契约

> 由 `npm run gen:spec-skills` 生成。这些技能已注册并实现，但不在 align 的武将技能表里
> （授予技、变体技能等）；语义以 `docs/games/sanguosha/skills.md` 与授予者的对齐单为准。

| 名称 | code | kind | 触发 | 窗口 | 主动 | 用例锚点 | 状态 |
|---|---|---|---|---|---|---|---|
| 攻心 | `gongxin` | normal | — | OwnPhase(Play) | ✔ | `tests/b10.rs::qinxue_grants_gongxin_and_gongxin_handles_heart_cards`<br>`tests/b10.rs::gongxin_can_put_heart_on_top_and_not_offered_without_heart` | implemented |
| 和弦 | `hexian` | normal | Healed | AnyTime | — | `tests/b8.rs::juexiang_grants_one_of_four_sub_skills_and_blocks_club_targets`<br>`tests/b8.rs::hexian_after_heal_heals_other_and_discards_their_equip` | implemented |
| 荐言 | `jianyan` | normal | — | OwnPhase(Play) | ✔ | `tests/b13.rs::qianxin_awaken_grants_jianyan`<br>`tests/b13.rs::jianyan_reveals_and_gives_to_male`<br>`tests/b13.rs::jianyan_needs_male` | implemented |
| 急袭 | `jixi` | normal | — | AnyTime | — | `tests/b15.rs::zaoxian_awaken_and_extra_turn`<br>`tests/b15.rs::zaoxian_grants_jixi_and_equipment_becomes_sha`<br>`tests/b15.rs::jixi_is_registered_and_grant_only` | implemented |
| 激弦 | `jixian` | normal | DamageAfter | AnyTime | — | `tests/b8.rs::juexiang_grants_one_of_four_sub_skills_and_blocks_club_targets`<br>`tests/b8.rs::jixian_after_damage_makes_source_lose_hp_and_use_equip` | implemented |
| 烈弦 | `liexian` | normal | Healed | AnyTime | — | `tests/b8.rs::juexiang_grants_one_of_four_sub_skills_and_blocks_club_targets`<br>`tests/b8.rs::liexian_after_heal_makes_other_lose_hp_and_use_equip` | implemented |
| 柔弦 | `rouxian` | normal | DamageAfter | AnyTime | — | `tests/b8.rs::juexiang_grants_one_of_four_sub_skills_and_blocks_club_targets`<br>`tests/b8.rs::rouxian_after_damage_heals_source_and_discards_their_equip` | implemented |
| 思蜀 | `sishu` | normal | PhaseStart | OwnPhase(Play) | — | `tests/b13.rs::ruoyu_awaken_strict_min_hp`<br>`tests/b13.rs::sishu_toggles_indulgence_judgement` | implemented |
