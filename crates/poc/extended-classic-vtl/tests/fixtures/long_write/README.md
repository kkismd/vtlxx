# 長い通常Write名の適合テスト

採用ADR [#453](https://github.com/kkismd/vtlxx/issues/453) と [#528](https://github.com/kkismd/vtlxx/issues/528) に基づき、通常Writeの名前と失敗時の状態を検査する。6502のテストはca65、ld65、sim65を必要とする。

| source | 両処理系の検査 | profile固有の検査 |
| --- | --- | --- |
| `success.vtl` | stdoutのraw bytes `abr8`。1/2/5/8文字、接頭辞、callee-first、`=()`、RHS、stack、IfElse/While | Rustは空stackも検査 |
| `invalid.vtl` | 各行を独立入力として空名、9文字、数字・大文字混在、重複、未定義、forward call、未完成ownerを拒否 | Rustは先行flushのstdout `ok`、A=7、完成済み `persist` の再利用と未完成 `open` の非公開を検査。6502はcompile失敗と空stdoutを検査 |

`long_write_conformance.rs` が上記を検査する。6502はwhole-program compile後に実行するため、失敗時に先行出力も実行しない。旧テンプレート構文は `common/removed_templates.rs` の共通入力をRust、6502 frontend、CLIの各テストで検査する。

`long_write_names.rs` は完成済み定義、Machineごとのbinding、既存builtin・label・named integerを検査する。`sim65_long_write_names.rs` は64定義・深さ64の成功、65定義目の失敗、名前とstack、複数chunkを検査する。arenaとowner/patchの容量・原子性は `sim65_backend.rs`、compile失敗時の実行抑止は `sim65_negative.rs` でも検査する。

過去の資源測定は [#456の監査](../../../../../../docs/ecvtl-6502-long-write-resources.md) と [PR #534](https://github.com/kkismd/vtlxx/pull/534) を参照。前者の構成・数値は測定当時のものであり現在の配置ではない。動的stack高水位やIRQ/NMIを含む実機の安全性はこのテストの測定対象ではない。
